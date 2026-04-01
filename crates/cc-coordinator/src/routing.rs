//! Permission-based tool routing for workers.
//!
//! Each worker can be granted access to a specific set of tools.
//! The `PermissionRouter` enforces these constraints and provides
//! a default set of tools that all workers inherit.

use std::collections::HashMap;

/// Routes tool access permissions per worker.
#[derive(Debug, Clone)]
pub struct PermissionRouter {
    /// Per-worker allowed tool names.
    worker_permissions: HashMap<String, Vec<String>>,
    /// Default tools granted to every worker.
    default_tools: Vec<String>,
}

impl PermissionRouter {
    /// Create a new empty permission router.
    pub fn new() -> Self {
        Self {
            worker_permissions: HashMap::new(),
            default_tools: Vec::new(),
        }
    }

    /// Set the allowed tools for a specific worker, replacing any previous list.
    pub fn set_worker_tools(&mut self, worker_id: &str, tools: Vec<String>) {
        self.worker_permissions
            .insert(worker_id.to_string(), tools);
    }

    /// Remove a worker's tool permissions entirely.
    pub fn remove_worker(&mut self, worker_id: &str) {
        self.worker_permissions.remove(worker_id);
    }

    /// Check whether a worker is allowed to use a given tool.
    ///
    /// Returns `true` if the tool is in the worker's explicit list
    /// or in the default tools.
    pub fn can_use_tool(&self, worker_id: &str, tool_name: &str) -> bool {
        // Check default tools first.
        if self.default_tools.iter().any(|t| t == tool_name) {
            return true;
        }
        // Check worker-specific tools.
        self.worker_permissions
            .get(worker_id)
            .map(|tools| tools.iter().any(|t| t == tool_name))
            .unwrap_or(false)
    }

    /// Get the full list of tools a worker is allowed to use,
    /// combining defaults with worker-specific grants.
    pub fn allowed_tools_for_worker(&self, worker_id: &str) -> Vec<String> {
        let mut tools = self.default_tools.clone();
        if let Some(worker_tools) = self.worker_permissions.get(worker_id) {
            for tool in worker_tools {
                if !tools.contains(tool) {
                    tools.push(tool.clone());
                }
            }
        }
        tools
    }

    /// Add a tool to the default set that all workers can use.
    pub fn add_default_tool(&mut self, tool: String) {
        if !self.default_tools.contains(&tool) {
            self.default_tools.push(tool);
        }
    }

    /// Returns the default tools list.
    pub fn default_tools(&self) -> &[String] {
        &self.default_tools
    }
}

impl Default for PermissionRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tools_apply_to_all_workers() {
        let mut router = PermissionRouter::new();
        router.add_default_tool("read".to_string());

        assert!(router.can_use_tool("worker-1", "read"));
        assert!(router.can_use_tool("worker-2", "read"));
        assert!(!router.can_use_tool("worker-1", "write"));
    }

    #[test]
    fn worker_specific_tools() {
        let mut router = PermissionRouter::new();
        router.set_worker_tools("w1", vec!["bash".to_string(), "edit".to_string()]);

        assert!(router.can_use_tool("w1", "bash"));
        assert!(router.can_use_tool("w1", "edit"));
        assert!(!router.can_use_tool("w1", "admin"));
        assert!(!router.can_use_tool("w2", "bash"));
    }

    #[test]
    fn allowed_tools_combines_defaults_and_specific() {
        let mut router = PermissionRouter::new();
        router.add_default_tool("read".to_string());
        router.set_worker_tools("w1", vec!["bash".to_string()]);

        let tools = router.allowed_tools_for_worker("w1");
        assert!(tools.contains(&"read".to_string()));
        assert!(tools.contains(&"bash".to_string()));
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn no_duplicate_defaults() {
        let mut router = PermissionRouter::new();
        router.add_default_tool("read".to_string());
        router.add_default_tool("read".to_string());
        assert_eq!(router.default_tools().len(), 1);
    }

    #[test]
    fn remove_worker_clears_permissions() {
        let mut router = PermissionRouter::new();
        router.set_worker_tools("w1", vec!["bash".to_string()]);
        assert!(router.can_use_tool("w1", "bash"));

        router.remove_worker("w1");
        assert!(!router.can_use_tool("w1", "bash"));
    }

    #[test]
    fn unknown_worker_gets_only_defaults() {
        let mut router = PermissionRouter::new();
        router.add_default_tool("read".to_string());

        let tools = router.allowed_tools_for_worker("nonexistent");
        assert_eq!(tools, vec!["read".to_string()]);
    }

    #[test]
    fn test_routing_default_tools() {
        let mut router = PermissionRouter::new();

        // Register the three standard default tools.
        router.add_default_tool("file_read".to_string());
        router.add_default_tool("grep".to_string());
        router.add_default_tool("glob".to_string());

        assert_eq!(router.default_tools().len(), 3);

        // Every worker (even unregistered ones) should have access to defaults.
        for worker_id in &["w1", "w2", "unknown"] {
            assert!(router.can_use_tool(worker_id, "file_read"));
            assert!(router.can_use_tool(worker_id, "grep"));
            assert!(router.can_use_tool(worker_id, "glob"));
            // But not a tool that is not in defaults.
            assert!(!router.can_use_tool(worker_id, "bash"));
        }

        // allowed_tools_for_worker should include all defaults.
        let tools = router.allowed_tools_for_worker("any-worker");
        assert!(tools.contains(&"file_read".to_string()));
        assert!(tools.contains(&"grep".to_string()));
        assert!(tools.contains(&"glob".to_string()));
    }

    #[test]
    fn test_routing_custom_tools() {
        let mut router = PermissionRouter::new();
        router.add_default_tool("file_read".to_string());

        // Give worker-a custom tools.
        router.set_worker_tools(
            "worker-a",
            vec!["bash".to_string(), "edit".to_string()],
        );

        // worker-a should have defaults + custom tools.
        assert!(router.can_use_tool("worker-a", "file_read"));
        assert!(router.can_use_tool("worker-a", "bash"));
        assert!(router.can_use_tool("worker-a", "edit"));
        assert!(!router.can_use_tool("worker-a", "admin"));

        let tools = router.allowed_tools_for_worker("worker-a");
        assert_eq!(tools.len(), 3); // file_read + bash + edit

        // worker-b has no custom tools, only defaults.
        assert!(router.can_use_tool("worker-b", "file_read"));
        assert!(!router.can_use_tool("worker-b", "bash"));
        assert!(!router.can_use_tool("worker-b", "edit"));

        // Updating worker-a's tools replaces the old list.
        router.set_worker_tools("worker-a", vec!["admin".to_string()]);
        assert!(router.can_use_tool("worker-a", "admin"));
        assert!(!router.can_use_tool("worker-a", "bash")); // bash no longer allowed
        assert!(router.can_use_tool("worker-a", "file_read")); // defaults still work
    }
}
