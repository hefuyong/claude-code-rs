//! Memory system for Claude Code RS.
//!
//! Scans for CLAUDE.md, MEMORY.md, and `.claude/memory/` files
//! to inject into the system prompt as persistent context.
//!
//! Also provides auto-extraction of memories from conversations
//! and periodic "dream" consolidation of stored memories.

pub mod dream;
pub mod extraction;

use std::path::{Path, PathBuf};

/// Maximum total size of memory content in bytes (25 KB).
const MAX_MEMORY_BYTES: usize = 25 * 1024;

/// Maximum number of lines to keep from a single memory file.
const MAX_LINES_PER_FILE: usize = 200;

/// A single memory entry loaded from disk.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    /// The file path this entry was loaded from.
    pub path: PathBuf,
    /// The text content of the file (possibly truncated).
    pub content: String,
    /// Where the memory came from.
    pub source: MemorySource,
}

/// The origin of a memory entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemorySource {
    /// A CLAUDE.md file found in the project tree.
    ClaudeMd,
    /// A file from the project-level `.claude/memory/` directory.
    ProjectMemory,
    /// A file from the user-level `~/.claude/memory/` directory.
    UserMemory,
}

/// Scans the project directory and user home for memory files.
pub struct MemoryScanner {
    project_root: PathBuf,
}

impl MemoryScanner {
    /// Create a new scanner rooted at the given project directory.
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Scan all known memory locations and return the entries found.
    pub async fn scan(&self) -> Vec<MemoryEntry> {
        let mut entries = Vec::new();

        // 1. CLAUDE.md in project root and parent directories.
        self.scan_claude_md(&mut entries).await;

        // 2. Project-level .claude/memory/ directory.
        let project_memory_dir = self.project_root.join(".claude").join("memory");
        self.scan_memory_dir(&project_memory_dir, MemorySource::ProjectMemory, &mut entries)
            .await;

        // 3. User-level ~/.claude/memory/ directory.
        if let Some(home) = dirs::home_dir() {
            let user_memory_dir = home.join(".claude").join("memory");
            self.scan_memory_dir(&user_memory_dir, MemorySource::UserMemory, &mut entries)
                .await;
        }

        tracing::debug!(count = entries.len(), "memory entries loaded");
        entries
    }

    /// Scan for CLAUDE.md files from the project root up to the filesystem root.
    async fn scan_claude_md(&self, entries: &mut Vec<MemoryEntry>) {
        let mut dir = Some(self.project_root.as_path());
        let mut depth = 0;
        // Walk up to 5 parent directories to avoid scanning too far.
        while let Some(d) = dir {
            if depth > 5 {
                break;
            }
            let candidate = d.join("CLAUDE.md");
            if let Some(entry) = Self::load_file(&candidate, MemorySource::ClaudeMd).await {
                entries.push(entry);
            }
            dir = d.parent();
            depth += 1;
        }
    }

    /// Scan a memory directory for `.md` and `.txt` files.
    async fn scan_memory_dir(
        &self,
        dir: &Path,
        source: MemorySource,
        entries: &mut Vec<MemoryEntry>,
    ) {
        let read_dir = match tokio::fs::read_dir(dir).await {
            Ok(rd) => rd,
            Err(_) => return, // directory doesn't exist, that's fine
        };

        let mut rd = read_dir;
        while let Ok(Some(entry)) = rd.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == "md" || ext == "txt" {
                    if let Some(mem_entry) = Self::load_file(&path, source.clone()).await {
                        entries.push(mem_entry);
                    }
                }
            }
        }
    }

    /// Load a single file, truncating to limits.
    async fn load_file(path: &Path, source: MemorySource) -> Option<MemoryEntry> {
        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) => {
                tracing::trace!(path = %path.display(), error = %e, "could not read memory file");
                return None;
            }
        };

        if content.trim().is_empty() {
            return None;
        }

        // Truncate by line count.
        let truncated: String = content
            .lines()
            .take(MAX_LINES_PER_FILE)
            .collect::<Vec<_>>()
            .join("\n");

        // Truncate by byte size.
        let final_content = if truncated.len() > MAX_MEMORY_BYTES {
            truncated[..MAX_MEMORY_BYTES].to_string()
        } else {
            truncated
        };

        tracing::debug!(
            path = %path.display(),
            bytes = final_content.len(),
            "loaded memory file"
        );

        Some(MemoryEntry {
            path: path.to_path_buf(),
            content: final_content,
            source,
        })
    }

    /// Format a set of memory entries into a string suitable for
    /// inclusion in the system prompt.
    pub fn format_for_prompt(entries: &[MemoryEntry]) -> String {
        if entries.is_empty() {
            return String::new();
        }

        let mut parts = Vec::new();
        let mut total_bytes = 0usize;

        for entry in entries {
            if total_bytes + entry.content.len() > MAX_MEMORY_BYTES {
                tracing::debug!(
                    total_bytes,
                    "memory budget exhausted, skipping remaining entries"
                );
                break;
            }

            let source_label = match entry.source {
                MemorySource::ClaudeMd => "CLAUDE.md",
                MemorySource::ProjectMemory => "project memory",
                MemorySource::UserMemory => "user memory",
            };

            parts.push(format!(
                "# Memory from {} ({})\n\n{}",
                source_label,
                entry.path.display(),
                entry.content
            ));

            total_bytes += entry.content.len();
        }

        parts.join("\n\n---\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_empty() {
        assert_eq!(MemoryScanner::format_for_prompt(&[]), "");
    }

    #[test]
    fn format_single_entry() {
        let entries = vec![MemoryEntry {
            path: PathBuf::from("/project/CLAUDE.md"),
            content: "Remember: use rust idioms".to_string(),
            source: MemorySource::ClaudeMd,
        }];
        let result = MemoryScanner::format_for_prompt(&entries);
        assert!(result.contains("CLAUDE.md"));
        assert!(result.contains("Remember: use rust idioms"));
    }

    #[test]
    fn format_multiple_entries() {
        let entries = vec![
            MemoryEntry {
                path: PathBuf::from("/project/CLAUDE.md"),
                content: "Project rules".to_string(),
                source: MemorySource::ClaudeMd,
            },
            MemoryEntry {
                path: PathBuf::from("/home/.claude/memory/prefs.md"),
                content: "User preferences".to_string(),
                source: MemorySource::UserMemory,
            },
        ];
        let result = MemoryScanner::format_for_prompt(&entries);
        assert!(result.contains("Project rules"));
        assert!(result.contains("User preferences"));
        assert!(result.contains("---"));
    }
}
