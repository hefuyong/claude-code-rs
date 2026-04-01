//! Worker agent management.
//!
//! A `Worker` represents a subprocess running a Claude agent
//! that has been delegated a specific task with a constrained tool set.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, error, info};
use uuid::Uuid;

use cc_error::{CcError, CcResult};

/// Configuration for spawning a new worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// A human-readable name for this worker.
    pub name: String,
    /// The type of agent to spawn (e.g. "code", "review", "test").
    pub agent_type: String,
    /// A description of the task this worker should perform.
    pub task_description: String,
    /// Tools that the worker is permitted to use.
    pub allowed_tools: Vec<String>,
    /// Optional model override for this worker.
    pub model: Option<String>,
    /// Isolation level for the worker's filesystem access.
    pub isolation: WorkerIsolation,
}

/// Filesystem isolation mode for a worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerIsolation {
    /// Worker shares the main working directory.
    Shared,
    /// Worker runs in an isolated git worktree.
    Worktree,
}

/// Lifecycle status of a worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerStatus {
    /// Worker is being initialized.
    Starting,
    /// Worker is actively running.
    Running,
    /// Worker finished its task successfully.
    Completed,
    /// Worker encountered an error.
    Failed,
    /// Worker was explicitly killed.
    Killed,
}

impl WorkerStatus {
    /// Returns `true` if this status represents a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            WorkerStatus::Completed | WorkerStatus::Failed | WorkerStatus::Killed
        )
    }
}

/// A running (or completed) worker agent subprocess.
pub struct Worker {
    /// Unique worker identifier.
    pub id: String,
    /// Configuration the worker was created with.
    pub config: WorkerConfig,
    /// Current status.
    pub status: WorkerStatus,
    /// Collected output lines from the worker.
    pub output: Vec<String>,
    /// When the worker was created.
    created_at: DateTime<Utc>,
    /// Handle to the subprocess, if running.
    process_handle: Option<Child>,
}

impl Worker {
    /// Create a new worker from the given configuration.
    ///
    /// The worker is created in `Starting` status and must be explicitly
    /// started with [`Worker::start`].
    pub fn new(config: WorkerConfig) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            config,
            status: WorkerStatus::Starting,
            output: Vec::new(),
            created_at: Utc::now(),
            process_handle: None,
        }
    }

    /// Spawn the worker subprocess.
    pub async fn start(&mut self) -> CcResult<()> {
        if self.status != WorkerStatus::Starting {
            return Err(CcError::Internal(format!(
                "Cannot start worker in {:?} status",
                self.status
            )));
        }

        info!(
            worker_id = %self.id,
            name = %self.config.name,
            "Starting worker"
        );

        let mut cmd = Command::new("claude-code");
        cmd.arg("--agent")
            .arg(&self.config.agent_type)
            .arg("--task")
            .arg(&self.config.task_description)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(ref model) = self.config.model {
            cmd.arg("--model").arg(model);
        }

        let child = cmd.spawn().map_err(|e| {
            self.status = WorkerStatus::Failed;
            CcError::Io(e)
        })?;

        self.process_handle = Some(child);
        self.status = WorkerStatus::Running;

        debug!(worker_id = %self.id, "Worker subprocess spawned");
        Ok(())
    }

    /// Kill the worker subprocess.
    pub async fn kill(&mut self) -> CcResult<()> {
        if let Some(ref mut child) = self.process_handle {
            child.kill().await.map_err(|e| {
                error!(worker_id = %self.id, "Failed to kill worker: {e}");
                CcError::Io(e)
            })?;
            info!(worker_id = %self.id, "Worker killed");
        }
        self.status = WorkerStatus::Killed;
        self.process_handle = None;
        Ok(())
    }

    /// Send input text to the worker's stdin.
    pub async fn send_input(&mut self, input: &str) -> CcResult<()> {
        let child = self.process_handle.as_mut().ok_or_else(|| {
            CcError::Internal("Worker has no running process".into())
        })?;

        let stdin = child.stdin.as_mut().ok_or_else(|| {
            CcError::Internal("Worker stdin not available".into())
        })?;

        stdin
            .write_all(input.as_bytes())
            .await
            .map_err(CcError::Io)?;
        stdin.write_all(b"\n").await.map_err(CcError::Io)?;
        stdin.flush().await.map_err(CcError::Io)?;

        Ok(())
    }

    /// Read the next line of output from the worker's stdout.
    ///
    /// Returns `None` if the stream is closed.
    pub async fn read_output(&mut self) -> CcResult<Option<String>> {
        let child = self.process_handle.as_mut().ok_or_else(|| {
            CcError::Internal("Worker has no running process".into())
        })?;

        let stdout = child.stdout.as_mut().ok_or_else(|| {
            CcError::Internal("Worker stdout not available".into())
        })?;

        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        match reader.read_line(&mut line).await {
            Ok(0) => {
                // EOF -- process likely exited.
                self.status = WorkerStatus::Completed;
                Ok(None)
            }
            Ok(_) => {
                let trimmed = line.trim_end().to_string();
                self.output.push(trimmed.clone());
                Ok(Some(trimmed))
            }
            Err(e) => {
                self.status = WorkerStatus::Failed;
                Err(CcError::Io(e))
            }
        }
    }

    /// Returns `true` if the worker subprocess is still running.
    pub fn is_alive(&self) -> bool {
        self.status == WorkerStatus::Running && self.process_handle.is_some()
    }

    /// Returns how long ago this worker was created.
    pub fn elapsed(&self) -> chrono::Duration {
        Utc::now() - self.created_at
    }

    /// Returns when this worker was created.
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> WorkerConfig {
        WorkerConfig {
            name: "test-worker".into(),
            agent_type: "code".into(),
            task_description: "Write a hello world program".into(),
            allowed_tools: vec!["bash".into(), "edit".into()],
            model: None,
            isolation: WorkerIsolation::Shared,
        }
    }

    #[test]
    fn new_worker_has_starting_status() {
        let worker = Worker::new(sample_config());
        assert_eq!(worker.status, WorkerStatus::Starting);
        assert!(!worker.is_alive());
        assert!(worker.output.is_empty());
    }

    #[test]
    fn worker_has_unique_id() {
        let w1 = Worker::new(sample_config());
        let w2 = Worker::new(sample_config());
        assert_ne!(w1.id, w2.id);
    }

    #[test]
    fn terminal_status_detection() {
        assert!(WorkerStatus::Completed.is_terminal());
        assert!(WorkerStatus::Failed.is_terminal());
        assert!(WorkerStatus::Killed.is_terminal());
        assert!(!WorkerStatus::Starting.is_terminal());
        assert!(!WorkerStatus::Running.is_terminal());
    }

    #[test]
    fn elapsed_is_non_negative() {
        let worker = Worker::new(sample_config());
        assert!(worker.elapsed().num_milliseconds() >= 0);
    }

    #[tokio::test]
    async fn kill_unstarted_worker_sets_killed() {
        let mut worker = Worker::new(sample_config());
        worker.kill().await.unwrap();
        assert_eq!(worker.status, WorkerStatus::Killed);
    }
}
