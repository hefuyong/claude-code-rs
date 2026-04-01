//! Permission controls for MCP server and tool access.
//!
//! Provides a simple allow/deny model for gating which MCP servers
//! may be connected to and which tools may be invoked.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Permission decision for a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolPermission {
    /// The tool may be used without prompting.
    Allow,
    /// The tool must not be used.
    Deny,
    /// The user should be prompted before using the tool.
    Ask,
}

/// Permission rules for MCP servers and their tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpPermissions {
    /// Servers that are explicitly allowed (empty means allow all).
    #[serde(default)]
    pub allowed_servers: Vec<String>,
    /// Servers that are explicitly denied.
    #[serde(default)]
    pub denied_servers: Vec<String>,
    /// Per-tool permission overrides, keyed as `"server_name/tool_name"`.
    #[serde(default)]
    pub tool_permissions: HashMap<String, ToolPermission>,
}

impl McpPermissions {
    /// Create a permissive configuration that allows everything.
    pub fn allow_all() -> Self {
        Self::default()
    }

    /// Check whether a server is allowed to be connected.
    ///
    /// A server is allowed if:
    /// - It is not in the denied list, AND
    /// - The allowed list is empty (allow all) or the server is in the allowed list.
    pub fn check_server(&self, name: &str) -> bool {
        if self.denied_servers.iter().any(|s| s == name) {
            return false;
        }
        if self.allowed_servers.is_empty() {
            return true;
        }
        self.allowed_servers.iter().any(|s| s == name)
    }

    /// Check the permission for a specific tool on a specific server.
    ///
    /// Looks up `"server/tool"` in the tool permissions map.
    /// Returns [`ToolPermission::Ask`] if no explicit rule exists.
    pub fn check_tool(&self, server: &str, tool: &str) -> ToolPermission {
        let key = format!("{server}/{tool}");
        self.tool_permissions
            .get(&key)
            .copied()
            .unwrap_or(ToolPermission::Ask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_all_permits_everything() {
        let perms = McpPermissions::allow_all();
        assert!(perms.check_server("anything"));
        assert_eq!(perms.check_tool("srv", "tool"), ToolPermission::Ask);
    }

    #[test]
    fn denied_server_is_blocked() {
        let perms = McpPermissions {
            denied_servers: vec!["evil-server".into()],
            ..Default::default()
        };
        assert!(!perms.check_server("evil-server"));
        assert!(perms.check_server("good-server"));
    }

    #[test]
    fn allowed_list_restricts_to_named_servers() {
        let perms = McpPermissions {
            allowed_servers: vec!["approved".into()],
            ..Default::default()
        };
        assert!(perms.check_server("approved"));
        assert!(!perms.check_server("unknown"));
    }

    #[test]
    fn denied_takes_precedence_over_allowed() {
        let perms = McpPermissions {
            allowed_servers: vec!["server-a".into()],
            denied_servers: vec!["server-a".into()],
            ..Default::default()
        };
        assert!(!perms.check_server("server-a"));
    }

    #[test]
    fn tool_permission_lookup() {
        let mut tool_permissions = HashMap::new();
        tool_permissions.insert("files/read".into(), ToolPermission::Allow);
        tool_permissions.insert("files/delete".into(), ToolPermission::Deny);

        let perms = McpPermissions {
            tool_permissions,
            ..Default::default()
        };

        assert_eq!(perms.check_tool("files", "read"), ToolPermission::Allow);
        assert_eq!(perms.check_tool("files", "delete"), ToolPermission::Deny);
        assert_eq!(perms.check_tool("files", "write"), ToolPermission::Ask);
    }
}
