//! Dream task — periodic memory consolidation.
//!
//! Scans the memory directory, detects duplicate or conflicting entries,
//! and merges / removes them to keep the memory set clean and focused.

use std::path::PathBuf;

use cc_error::{CcError, CcResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::MemoryEntry;

/// Status of a dream (consolidation) task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DreamStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

/// A single dream consolidation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamTask {
    /// Unique identifier for this run.
    pub id: String,
    /// Current status.
    pub status: DreamStatus,
    /// When the task was created.
    pub started_at: DateTime<Utc>,
    /// When it finished (if applicable).
    pub finished_at: Option<DateTime<Utc>>,
    /// Number of actions performed.
    pub actions_taken: usize,
}

/// Configuration for the dream runner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamConfig {
    /// Whether dream consolidation is enabled.
    pub enabled: bool,
    /// Hours between automatic runs.
    pub interval_hours: u32,
    /// Maximum number of memory entries to process per run.
    pub max_memories: usize,
}

impl Default for DreamConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_hours: 24,
            max_memories: 200,
        }
    }
}

/// An action the dream runner will take on the memory store.
#[derive(Debug, Clone)]
pub enum ConsolidationAction {
    /// Merge multiple memory files into a single file.
    Merge {
        sources: Vec<PathBuf>,
        into: PathBuf,
    },
    /// Remove a memory file.
    Remove {
        path: PathBuf,
        reason: String,
    },
    /// Rewrite a memory file with updated content.
    Update {
        path: PathBuf,
        new_content: String,
    },
}

/// Timestamp file used to track when the last dream run occurred.
const LAST_RUN_FILE: &str = ".claude_dream_last_run";

/// Runs periodic memory consolidation.
pub struct DreamRunner {
    config: DreamConfig,
    memory_dir: PathBuf,
}

impl DreamRunner {
    /// Create a new dream runner.
    pub fn new(config: DreamConfig, memory_dir: PathBuf) -> Self {
        Self { config, memory_dir }
    }

    /// Check whether enough time has passed since the last run.
    pub fn should_run(&self) -> bool {
        if !self.config.enabled {
            return false;
        }
        let last_run_path = self.memory_dir.join(LAST_RUN_FILE);
        let last_run = match std::fs::read_to_string(&last_run_path) {
            Ok(s) => s.trim().parse::<DateTime<Utc>>().ok(),
            Err(_) => None,
        };

        match last_run {
            Some(ts) => {
                let elapsed = Utc::now() - ts;
                elapsed.num_hours() >= self.config.interval_hours as i64
            }
            None => true, // never run before
        }
    }

    /// Execute a consolidation run.
    pub async fn run_consolidation(&self) -> CcResult<DreamTask> {
        let id = uuid::Uuid::new_v4().to_string();
        let mut task = DreamTask {
            id,
            status: DreamStatus::Running,
            started_at: Utc::now(),
            finished_at: None,
            actions_taken: 0,
        };

        match self.consolidate_memories().await {
            Ok(actions) => {
                task.actions_taken = actions.len();
                for action in &actions {
                    if let Err(e) = self.apply_action(action).await {
                        tracing::warn!(error = %e, "failed to apply consolidation action");
                    }
                }
                task.status = DreamStatus::Completed;
            }
            Err(e) => {
                tracing::error!(error = %e, "dream consolidation failed");
                task.status = DreamStatus::Failed;
            }
        }

        task.finished_at = Some(Utc::now());

        // Record the last-run timestamp.
        let ts_path = self.memory_dir.join(LAST_RUN_FILE);
        let _ = tokio::fs::write(&ts_path, Utc::now().to_rfc3339()).await;

        tracing::info!(
            task_id = %task.id,
            actions = task.actions_taken,
            status = ?task.status,
            "dream consolidation finished"
        );

        Ok(task)
    }

    /// Scan memory files and determine what consolidation actions to take.
    async fn consolidate_memories(&self) -> CcResult<Vec<ConsolidationAction>> {
        let memories = self.load_all_memories().await?;
        let mut actions = Vec::new();

        // Phase 1: deduplication
        let deduped = self.deduplicate(&memories);
        let removed_count = memories.len() - deduped.len();
        if removed_count > 0 {
            // Find which entries were removed and create Remove actions.
            for mem in &memories {
                if !deduped.iter().any(|d| d.path == mem.path) {
                    actions.push(ConsolidationAction::Remove {
                        path: mem.path.clone(),
                        reason: "duplicate content".to_string(),
                    });
                }
            }
        }

        tracing::debug!(
            total = memories.len(),
            duplicates = removed_count,
            "consolidation analysis complete"
        );

        Ok(actions)
    }

    /// Remove entries whose content is a substring of another entry or
    /// whose content hashes match.
    fn deduplicate(&self, memories: &[MemoryEntry]) -> Vec<MemoryEntry> {
        let mut result: Vec<MemoryEntry> = Vec::new();
        let limit = memories.len().min(self.config.max_memories);

        for mem in memories.iter().take(limit) {
            let dominated = result.iter().any(|existing| {
                Self::normalized(existing).contains(&Self::normalized(mem))
            });
            let dominates = result.iter().any(|existing| {
                Self::normalized(mem).contains(&Self::normalized(existing))
            });

            if !dominated {
                if dominates {
                    // Remove the smaller entry from result; keep the larger one.
                    result.retain(|existing| {
                        !Self::normalized(mem).contains(&Self::normalized(existing))
                    });
                }
                result.push(mem.clone());
            }
        }

        result
    }

    /// Normalize content for comparison (lowercase, collapse whitespace).
    fn normalized(entry: &MemoryEntry) -> String {
        entry
            .content
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Load all memory files from the memory directory.
    async fn load_all_memories(&self) -> CcResult<Vec<MemoryEntry>> {
        let mut entries = Vec::new();
        let mut rd = tokio::fs::read_dir(&self.memory_dir)
            .await
            .map_err(|e| CcError::Io(e))?;

        while let Ok(Some(entry)) = rd.next_entry().await {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if ext != "md" && ext != "txt" {
                continue;
            }
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                entries.push(MemoryEntry {
                    path,
                    content,
                    source: crate::MemorySource::ProjectMemory,
                });
            }
        }

        Ok(entries)
    }

    /// Apply a single consolidation action to the filesystem.
    async fn apply_action(&self, action: &ConsolidationAction) -> CcResult<()> {
        match action {
            ConsolidationAction::Remove { path, reason } => {
                tracing::debug!(path = %path.display(), reason, "removing memory file");
                tokio::fs::remove_file(path)
                    .await
                    .map_err(|e| CcError::Io(e))?;
            }
            ConsolidationAction::Update { path, new_content } => {
                tracing::debug!(path = %path.display(), "updating memory file");
                tokio::fs::write(path, new_content)
                    .await
                    .map_err(|e| CcError::Io(e))?;
            }
            ConsolidationAction::Merge { sources, into } => {
                tracing::debug!(into = %into.display(), count = sources.len(), "merging memories");
                let mut combined = String::new();
                for src in sources {
                    if let Ok(content) = tokio::fs::read_to_string(src).await {
                        if !combined.is_empty() {
                            combined.push_str("\n\n---\n\n");
                        }
                        combined.push_str(&content);
                    }
                }
                tokio::fs::write(into, &combined)
                    .await
                    .map_err(|e| CcError::Io(e))?;
                // Remove originals (except the target).
                for src in sources {
                    if src != into {
                        let _ = tokio::fs::remove_file(src).await;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemorySource;

    fn mem_entry(path: &str, content: &str) -> MemoryEntry {
        MemoryEntry {
            path: PathBuf::from(path),
            content: content.to_string(),
            source: MemorySource::ProjectMemory,
        }
    }

    #[test]
    fn default_config() {
        let cfg = DreamConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.interval_hours, 24);
        assert_eq!(cfg.max_memories, 200);
    }

    #[test]
    fn should_run_when_never_run() {
        let runner = DreamRunner::new(
            DreamConfig::default(),
            PathBuf::from("/nonexistent/path"),
        );
        assert!(runner.should_run());
    }

    #[test]
    fn should_not_run_when_disabled() {
        let mut cfg = DreamConfig::default();
        cfg.enabled = false;
        let runner = DreamRunner::new(cfg, PathBuf::from("/tmp"));
        assert!(!runner.should_run());
    }

    #[test]
    fn deduplicate_removes_subsets() {
        let runner = DreamRunner::new(DreamConfig::default(), PathBuf::from("/tmp"));
        let memories = vec![
            mem_entry("/a.md", "use rust idioms"),
            mem_entry("/b.md", "always use rust idioms when writing code"),
            mem_entry("/c.md", "prefer tabs"),
        ];
        let result = runner.deduplicate(&memories);
        // "use rust idioms" is a subset of the longer entry, so it should be removed.
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|m| m.content.contains("always")));
        assert!(result.iter().any(|m| m.content.contains("tabs")));
    }

    #[test]
    fn deduplicate_keeps_unique() {
        let runner = DreamRunner::new(DreamConfig::default(), PathBuf::from("/tmp"));
        let memories = vec![
            mem_entry("/a.md", "prefer spaces"),
            mem_entry("/b.md", "use postgresql"),
        ];
        let result = runner.deduplicate(&memories);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn dream_task_status_serde() {
        let task = DreamTask {
            id: "test-1".into(),
            status: DreamStatus::Completed,
            started_at: Utc::now(),
            finished_at: Some(Utc::now()),
            actions_taken: 3,
        };
        let json = serde_json::to_string(&task).unwrap();
        let parsed: DreamTask = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, DreamStatus::Completed);
        assert_eq!(parsed.actions_taken, 3);
    }
}
