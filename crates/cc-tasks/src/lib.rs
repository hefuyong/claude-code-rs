//! Background task management for Claude Code RS.
//!
//! Tracks the lifecycle of background tasks such as local bash
//! commands or agent sub-tasks.

use cc_error::{CcError, CcResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

// ── Types ───────────────────────────────────────────────────────────

/// The kind of background task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// A local shell command.
    LocalBash,
    /// A sub-agent task.
    LocalAgent,
}

/// Current status of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Waiting to start.
    Pending,
    /// Currently executing.
    Running,
    /// Finished successfully.
    Completed,
    /// Finished with an error.
    Failed,
    /// Forcibly terminated.
    Killed,
}

/// Information about a single task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    /// Unique task identifier.
    pub id: String,
    /// The kind of task.
    pub task_type: TaskType,
    /// Current status.
    pub status: TaskStatus,
    /// Optional path to the output file.
    pub output_file: Option<PathBuf>,
    /// Optional description of the task.
    pub description: Option<String>,
    /// When the task was created (Unix millis).
    pub created_at: u64,
}

// ── Manager ─────────────────────────────────────────────────────────

/// Manages the lifecycle of background tasks.
pub struct TaskManager {
    tasks: HashMap<String, TaskInfo>,
}

impl TaskManager {
    /// Create a new task manager.
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    /// Create a new task and return its ID.
    pub fn create(&mut self, task_type: TaskType) -> String {
        let id = Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let info = TaskInfo {
            id: id.clone(),
            task_type,
            status: TaskStatus::Pending,
            output_file: None,
            description: None,
            created_at: now,
        };

        tracing::debug!(task_id = %id, task_type = ?task_type, "task created");
        self.tasks.insert(id.clone(), info);
        id
    }

    /// Create a task with a description and return its ID.
    pub fn create_with_description(
        &mut self,
        task_type: TaskType,
        description: impl Into<String>,
    ) -> String {
        let id = self.create(task_type);
        if let Some(task) = self.tasks.get_mut(&id) {
            task.description = Some(description.into());
        }
        id
    }

    /// Update the status of a task.
    pub fn update_status(&mut self, id: &str, status: TaskStatus) {
        if let Some(task) = self.tasks.get_mut(id) {
            tracing::debug!(task_id = id, old = ?task.status, new = ?status, "task status updated");
            task.status = status;
        } else {
            tracing::warn!(task_id = id, "attempted to update unknown task");
        }
    }

    /// Set the output file for a task.
    pub fn set_output_file(&mut self, id: &str, path: PathBuf) {
        if let Some(task) = self.tasks.get_mut(id) {
            task.output_file = Some(path);
        }
    }

    /// Get a task by ID.
    pub fn get(&self, id: &str) -> Option<&TaskInfo> {
        self.tasks.get(id)
    }

    /// List all tasks.
    pub fn list(&self) -> Vec<&TaskInfo> {
        let mut tasks: Vec<&TaskInfo> = self.tasks.values().collect();
        tasks.sort_by_key(|t| t.created_at);
        tasks
    }

    /// List tasks filtered by status.
    pub fn list_by_status(&self, status: TaskStatus) -> Vec<&TaskInfo> {
        self.tasks
            .values()
            .filter(|t| t.status == status)
            .collect()
    }

    /// Remove a completed or failed task.
    pub fn remove(&mut self, id: &str) -> CcResult<()> {
        let task = self
            .tasks
            .get(id)
            .ok_or_else(|| CcError::NotFound(format!("task '{id}' not found")))?;

        match task.status {
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Killed => {
                self.tasks.remove(id);
                Ok(())
            }
            _ => Err(CcError::Internal(format!(
                "cannot remove task '{id}' with status {:?}",
                task.status
            ))),
        }
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get() {
        let mut mgr = TaskManager::new();
        let id = mgr.create(TaskType::LocalBash);
        let task = mgr.get(&id).unwrap();
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.task_type, TaskType::LocalBash);
    }

    #[test]
    fn update_status() {
        let mut mgr = TaskManager::new();
        let id = mgr.create(TaskType::LocalAgent);
        mgr.update_status(&id, TaskStatus::Running);
        assert_eq!(mgr.get(&id).unwrap().status, TaskStatus::Running);
        mgr.update_status(&id, TaskStatus::Completed);
        assert_eq!(mgr.get(&id).unwrap().status, TaskStatus::Completed);
    }

    #[test]
    fn list_tasks() {
        let mut mgr = TaskManager::new();
        mgr.create(TaskType::LocalBash);
        mgr.create(TaskType::LocalAgent);
        assert_eq!(mgr.list().len(), 2);
    }

    #[test]
    fn remove_completed_task() {
        let mut mgr = TaskManager::new();
        let id = mgr.create(TaskType::LocalBash);
        mgr.update_status(&id, TaskStatus::Completed);
        assert!(mgr.remove(&id).is_ok());
        assert!(mgr.get(&id).is_none());
    }

    #[test]
    fn cannot_remove_running_task() {
        let mut mgr = TaskManager::new();
        let id = mgr.create(TaskType::LocalBash);
        mgr.update_status(&id, TaskStatus::Running);
        assert!(mgr.remove(&id).is_err());
    }

    #[test]
    fn list_by_status() {
        let mut mgr = TaskManager::new();
        let id1 = mgr.create(TaskType::LocalBash);
        let _id2 = mgr.create(TaskType::LocalBash);
        mgr.update_status(&id1, TaskStatus::Running);
        assert_eq!(mgr.list_by_status(TaskStatus::Running).len(), 1);
        assert_eq!(mgr.list_by_status(TaskStatus::Pending).len(), 1);
    }

    #[test]
    fn task_serialization() {
        let info = TaskInfo {
            id: "test-id".into(),
            task_type: TaskType::LocalBash,
            status: TaskStatus::Pending,
            output_file: None,
            description: Some("Test task".into()),
            created_at: 1234567890,
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: TaskInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "test-id");
        assert_eq!(back.task_type, TaskType::LocalBash);
    }
}
