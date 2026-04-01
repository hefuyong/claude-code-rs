//! The main coordinator for multi-agent orchestration.
//!
//! `Coordinator` manages a pool of workers, delegates tool calls,
//! and enforces permission constraints via the routing layer.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use cc_error::{CcError, CcResult};

use crate::routing::PermissionRouter;
use crate::worker::{Worker, WorkerConfig, WorkerStatus};

/// Summary information about a worker, suitable for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    /// Unique worker identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Current status.
    pub status: WorkerStatus,
    /// Description of what the worker is doing.
    pub task_description: String,
    /// When the worker was created.
    pub created_at: DateTime<Utc>,
}

/// The coordinator that manages multi-agent collaboration.
///
/// It maintains a pool of worker agents, each assigned a specific
/// task with constrained tool access. The coordinator is responsible
/// for spawning, monitoring, and cleaning up workers.
pub struct Coordinator {
    /// Active workers keyed by their ID.
    workers: HashMap<String, Worker>,
    /// Permission router that controls tool access per worker.
    permission_router: PermissionRouter,
    /// Maximum number of concurrent workers.
    max_workers: usize,
    /// Whether the coordinator is actively accepting new work.
    enabled: bool,
}

impl Coordinator {
    /// Create a new coordinator with a maximum worker count.
    pub fn new(max_workers: usize) -> Self {
        Self {
            workers: HashMap::new(),
            permission_router: PermissionRouter::new(),
            max_workers,
            enabled: false,
        }
    }

    /// Enable the coordinator for accepting work.
    pub fn enable(&mut self) {
        self.enabled = true;
        info!(
            "Multi-agent coordination enabled (max workers: {})",
            self.max_workers
        );
    }

    /// Disable the coordinator.
    ///
    /// Running workers are not killed -- use [`kill_all`](Self::kill_all) first if needed.
    pub fn disable(&mut self) {
        self.enabled = false;
        info!("Multi-agent coordination disabled");
    }

    /// Whether multi-agent coordination is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Spawn a new worker with the given configuration.
    ///
    /// Returns the worker's unique ID on success.
    pub async fn spawn_worker(&mut self, config: WorkerConfig) -> CcResult<String> {
        if !self.enabled {
            return Err(CcError::Internal(
                "Coordinator is not enabled -- call enable() first".into(),
            ));
        }

        if self.active_count() >= self.max_workers {
            return Err(CcError::Internal(format!(
                "Maximum worker count ({}) reached",
                self.max_workers
            )));
        }

        let mut worker = Worker::new(config.clone());
        let worker_id = worker.id.clone();

        // Register tool permissions for this worker.
        self.permission_router
            .set_worker_tools(&worker_id, config.allowed_tools.clone());

        worker.start().await?;

        info!(
            worker_id = %worker_id,
            name = %config.name,
            "Worker spawned"
        );

        self.workers.insert(worker_id.clone(), worker);
        Ok(worker_id)
    }

    /// Kill a specific worker by ID.
    pub async fn kill_worker(&mut self, id: &str) -> CcResult<()> {
        let worker = self.workers.get_mut(id).ok_or_else(|| {
            CcError::NotFound(format!("Worker '{id}' not found"))
        })?;

        worker.kill().await?;
        self.permission_router.remove_worker(id);
        debug!(worker_id = id, "Worker killed and permissions removed");
        Ok(())
    }

    /// Kill all running workers.
    pub async fn kill_all(&mut self) {
        let ids: Vec<String> = self
            .workers
            .iter()
            .filter(|(_, w)| w.is_alive())
            .map(|(id, _)| id.clone())
            .collect();

        for id in &ids {
            if let Some(worker) = self.workers.get_mut(id) {
                if let Err(e) = worker.kill().await {
                    warn!(worker_id = %id, error = %e, "Failed to kill worker");
                }
            }
            self.permission_router.remove_worker(id);
        }

        info!("All workers killed");
    }

    /// List summary information for all workers.
    pub fn list_workers(&self) -> Vec<WorkerInfo> {
        self.workers
            .values()
            .map(|w| WorkerInfo {
                id: w.id.clone(),
                name: w.config.name.clone(),
                status: w.status,
                task_description: w.config.task_description.clone(),
                created_at: w.created_at(),
            })
            .collect()
    }

    /// Get a reference to a worker by ID.
    pub fn get_worker(&self, id: &str) -> Option<&Worker> {
        self.workers.get(id)
    }

    /// Get a mutable reference to a worker by ID.
    pub fn get_worker_mut(&mut self, id: &str) -> Option<&mut Worker> {
        self.workers.get_mut(id)
    }

    /// Delegate a tool call to a specific worker, checking permissions first.
    ///
    /// Returns the tool output as a string.
    pub async fn delegate_tool_call(
        &mut self,
        worker_id: &str,
        tool_name: &str,
        input: serde_json::Value,
    ) -> CcResult<String> {
        // Check permission.
        if !self.permission_router.can_use_tool(worker_id, tool_name) {
            return Err(CcError::PermissionDenied(format!(
                "Worker '{worker_id}' is not allowed to use tool '{tool_name}'"
            )));
        }

        let worker = self.workers.get_mut(worker_id).ok_or_else(|| {
            CcError::NotFound(format!("Worker '{worker_id}' not found"))
        })?;

        if !worker.is_alive() {
            return Err(CcError::Internal(format!(
                "Worker '{worker_id}' is not running (status: {:?})",
                worker.status
            )));
        }

        // Send the tool call as a JSON message to the worker's stdin.
        let message = serde_json::json!({
            "type": "tool_call",
            "tool": tool_name,
            "input": input,
        });
        let message_str = serde_json::to_string(&message).map_err(|e| {
            CcError::Serialization(format!("Failed to serialize tool call: {e}"))
        })?;

        worker.send_input(&message_str).await?;

        // Read the worker's response.
        match worker.read_output().await? {
            Some(output) => Ok(output),
            None => {
                warn!(worker_id = %worker_id, "Worker closed stdout during tool call");
                Err(CcError::Internal(format!(
                    "Worker '{worker_id}' exited during tool call"
                )))
            }
        }
    }

    /// Broadcast a text message to all running workers.
    pub async fn broadcast_message(&mut self, message: &str) -> CcResult<()> {
        let ids: Vec<String> = self
            .workers
            .iter()
            .filter(|(_, w)| w.is_alive())
            .map(|(id, _)| id.clone())
            .collect();

        for id in &ids {
            if let Some(worker) = self.workers.get_mut(id) {
                if let Err(e) = worker.send_input(message).await {
                    warn!(worker_id = %id, "Failed to send broadcast to worker: {e}");
                }
            }
        }

        Ok(())
    }

    /// Returns the number of workers that are still alive.
    pub fn active_count(&self) -> usize {
        self.workers.values().filter(|w| w.is_alive()).count()
    }

    /// Total number of workers (including completed/failed).
    pub fn total_count(&self) -> usize {
        self.workers.len()
    }

    /// Generate a coordinator-specific system prompt describing available workers.
    pub fn get_system_prompt(&self) -> String {
        let mut prompt = String::from(
            "You are a coordinator managing multiple worker agents. \
             Available workers:\n\n",
        );

        if self.workers.is_empty() {
            prompt.push_str("  (no workers spawned)\n");
        } else {
            for worker in self.workers.values() {
                prompt.push_str(&format!(
                    "- {} [{}] (status: {:?}): {}\n",
                    worker.config.name,
                    worker.id,
                    worker.status,
                    worker.config.task_description,
                ));
            }
        }

        prompt.push_str(&format!(
            "\nActive: {}, Max: {}\n",
            self.active_count(),
            self.max_workers
        ));

        prompt
    }

    /// Access the permission router.
    pub fn permission_router(&self) -> &PermissionRouter {
        &self.permission_router
    }

    /// Access the permission router mutably.
    pub fn permission_router_mut(&mut self) -> &mut PermissionRouter {
        &mut self.permission_router
    }

    /// Remove all workers in terminal states (Completed, Failed, Killed).
    pub fn prune_finished(&mut self) -> usize {
        let before = self.workers.len();
        self.workers.retain(|_, w| !w.status.is_terminal());
        let removed = before - self.workers.len();
        if removed > 0 {
            info!("Pruned {removed} finished workers");
        }
        removed
    }

    /// Set the maximum number of concurrent workers.
    pub fn set_max_workers(&mut self, max: usize) {
        self.max_workers = max;
    }
}

impl Default for Coordinator {
    fn default() -> Self {
        Self::new(4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worker::WorkerIsolation;

    fn sample_config(name: &str) -> WorkerConfig {
        WorkerConfig {
            name: name.to_string(),
            agent_type: "code".into(),
            task_description: format!("Task for {name}"),
            allowed_tools: vec!["bash".into()],
            model: None,
            isolation: WorkerIsolation::Shared,
        }
    }

    #[test]
    fn new_coordinator_is_disabled() {
        let coord = Coordinator::new(4);
        assert!(!coord.is_enabled());
        assert_eq!(coord.active_count(), 0);
        assert_eq!(coord.total_count(), 0);
    }

    #[test]
    fn enable_disable_toggle() {
        let mut coord = Coordinator::new(4);
        coord.enable();
        assert!(coord.is_enabled());
        coord.disable();
        assert!(!coord.is_enabled());
    }

    #[tokio::test]
    async fn spawn_while_disabled_fails() {
        let mut coord = Coordinator::new(4);
        let result = coord.spawn_worker(sample_config("w1")).await;
        assert!(result.is_err());
    }

    #[test]
    fn list_workers_empty() {
        let coord = Coordinator::new(4);
        assert!(coord.list_workers().is_empty());
    }

    #[test]
    fn system_prompt_with_no_workers() {
        let coord = Coordinator::new(4);
        let prompt = coord.get_system_prompt();
        assert!(prompt.contains("no workers spawned"));
        assert!(prompt.contains("Active: 0"));
        assert!(prompt.contains("Max: 4"));
    }

    #[test]
    fn default_coordinator_has_max_4() {
        let coord = Coordinator::default();
        assert_eq!(coord.max_workers, 4);
        assert!(!coord.is_enabled());
    }

    #[test]
    fn prune_finished_on_empty() {
        let mut coord = Coordinator::new(4);
        assert_eq!(coord.prune_finished(), 0);
    }

    #[tokio::test]
    async fn kill_nonexistent_worker_fails() {
        let mut coord = Coordinator::new(4);
        let result = coord.kill_worker("nonexistent").await;
        assert!(result.is_err());
    }

    #[test]
    fn prune_removes_terminal_workers() {
        let mut coord = Coordinator::new(4);
        let mut w = Worker::new(WorkerConfig {
            name: "done-worker".into(),
            agent_type: "code".into(),
            task_description: "finished task".into(),
            allowed_tools: vec![],
            model: None,
            isolation: WorkerIsolation::Shared,
        });
        w.status = WorkerStatus::Completed;
        let id = w.id.clone();
        coord.workers.insert(id, w);

        assert_eq!(coord.total_count(), 1);
        assert_eq!(coord.active_count(), 0); // completed is not alive
        let removed = coord.prune_finished();
        assert_eq!(removed, 1);
        assert_eq!(coord.total_count(), 0);
    }

    #[test]
    fn set_max_workers() {
        let mut coord = Coordinator::new(2);
        assert_eq!(coord.max_workers, 2);
        coord.set_max_workers(8);
        assert_eq!(coord.max_workers, 8);
    }

    #[test]
    fn permission_router_access() {
        let mut coord = Coordinator::new(4);
        coord.permission_router_mut().add_default_tool("read".into());
        assert!(coord.permission_router().can_use_tool("any-worker", "read"));
    }

    #[test]
    fn test_spawn_and_list() {
        // Directly insert a worker to simulate spawn (since actually spawning
        // requires the claude-code binary). Verify it appears in list_workers.
        let mut coord = Coordinator::new(4);
        coord.enable();

        let config = sample_config("test-worker");
        let worker = Worker::new(config.clone());
        let id = worker.id.clone();
        // Worker starts in Starting status (no process handle yet).
        coord.workers.insert(id.clone(), worker);

        let list = coord.list_workers();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "test-worker");
        assert_eq!(list[0].status, WorkerStatus::Starting);
        assert_eq!(list[0].id, id);
        assert_eq!(list[0].task_description, "Task for test-worker");

        // get_worker returns the same worker.
        let w = coord.get_worker(&id).unwrap();
        assert_eq!(w.config.name, "test-worker");
        assert_eq!(w.config.agent_type, "code");
        assert_eq!(coord.total_count(), 1);
    }

    #[tokio::test]
    async fn test_kill_worker() {
        let mut coord = Coordinator::new(4);
        coord.enable();

        // Insert a worker in Starting status (no real process).
        let config = sample_config("killable");
        let worker = Worker::new(config);
        let id = worker.id.clone();
        coord.workers.insert(id.clone(), worker);
        coord
            .permission_router
            .set_worker_tools(&id, vec!["bash".into()]);

        assert_eq!(coord.total_count(), 1);

        // Kill the worker. Since there is no process_handle, kill()
        // just sets status to Killed without errors.
        coord.kill_worker(&id).await.unwrap();

        // Verify status is Killed and permissions were removed.
        let w = coord.get_worker(&id).unwrap();
        assert_eq!(w.status, WorkerStatus::Killed);
        assert!(!w.is_alive());
        assert!(w.status.is_terminal());
        // Permissions should be removed.
        assert!(!coord.permission_router().can_use_tool(&id, "bash"));
    }

    #[tokio::test]
    async fn test_max_workers_limit() {
        // With max_workers = 0, any spawn attempt should be rejected.
        let mut coord = Coordinator::new(0);
        coord.enable();

        let result = coord.spawn_worker(sample_config("w1")).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("Maximum worker count"),
            "Expected max workers error, got: {err_msg}"
        );

        // After increasing the limit, the guard should pass.
        coord.set_max_workers(1);
        let result = coord.spawn_worker(sample_config("w2")).await;
        // Either succeeds (binary found) or fails at process spawn (not limit).
        match &result {
            Ok(_) => {
                // Binary found, spawn succeeded — that's fine.
                assert_eq!(coord.list_workers().len(), 1);
            }
            Err(e) => {
                // Should NOT be a "Maximum worker count" error.
                let err2 = format!("{e}");
                assert!(
                    !err2.contains("Maximum worker count"),
                    "Should have passed the limit check, got: {err2}"
                );
            }
        }
    }
}
