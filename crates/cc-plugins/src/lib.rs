//! Plugin management for Claude Code RS.
//!
//! Plugins can provide additional MCP servers, tools, and capabilities.
//! They are loaded from configuration files and managed at runtime.

use cc_error::{CcError, CcResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ── Types ───────────────────────────────────────────────────────────

/// A plugin definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plugin {
    /// Unique plugin name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Whether this plugin is currently enabled.
    pub enabled: bool,
    /// MCP servers provided by this plugin.
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

/// Configuration for an MCP server bundled with a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name.
    pub name: String,
    /// Command to launch the server.
    pub command: String,
    /// Arguments for the command.
    #[serde(default)]
    pub args: Vec<String>,
}

/// Configuration file format for loading plugins.
#[derive(Debug, Deserialize)]
struct PluginConfigFile {
    #[serde(default)]
    plugins: Vec<Plugin>,
}

// ── Manager ─────────────────────────────────────────────────────────

/// Manages the lifecycle of plugins.
pub struct PluginManager {
    plugins: Vec<Plugin>,
}

impl PluginManager {
    /// Create an empty plugin manager.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Load plugins from a JSON config file.
    pub async fn load_from_config(&mut self, config_path: &Path) -> CcResult<()> {
        let content = tokio::fs::read_to_string(config_path)
            .await
            .map_err(|e| {
                CcError::Config(format!(
                    "failed to read plugin config {}: {}",
                    config_path.display(),
                    e
                ))
            })?;

        let config: PluginConfigFile = serde_json::from_str(&content)
            .map_err(|e| CcError::Config(format!("invalid plugin config: {e}")))?;

        for plugin in config.plugins {
            tracing::debug!(name = %plugin.name, enabled = plugin.enabled, "loaded plugin");
            self.plugins.push(plugin);
        }

        Ok(())
    }

    /// Return all loaded plugins.
    pub fn list(&self) -> &[Plugin] {
        &self.plugins
    }

    /// Enable a plugin by name.
    pub fn enable(&mut self, name: &str) -> CcResult<()> {
        let plugin = self
            .plugins
            .iter_mut()
            .find(|p| p.name == name)
            .ok_or_else(|| CcError::NotFound(format!("plugin '{name}' not found")))?;
        plugin.enabled = true;
        tracing::info!(name, "plugin enabled");
        Ok(())
    }

    /// Disable a plugin by name.
    pub fn disable(&mut self, name: &str) -> CcResult<()> {
        let plugin = self
            .plugins
            .iter_mut()
            .find(|p| p.name == name)
            .ok_or_else(|| CcError::NotFound(format!("plugin '{name}' not found")))?;
        plugin.enabled = false;
        tracing::info!(name, "plugin disabled");
        Ok(())
    }

    /// Get only the enabled plugins.
    pub fn enabled_plugins(&self) -> Vec<&Plugin> {
        self.plugins.iter().filter(|p| p.enabled).collect()
    }

    /// Collect all MCP server configs from enabled plugins.
    pub fn mcp_server_configs(&self) -> Vec<&McpServerConfig> {
        self.enabled_plugins()
            .iter()
            .flat_map(|p| p.mcp_servers.iter())
            .collect()
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_disable() {
        let mut mgr = PluginManager::new();
        mgr.plugins.push(Plugin {
            name: "test-plugin".into(),
            description: "A test plugin".into(),
            enabled: false,
            mcp_servers: vec![],
        });

        assert_eq!(mgr.enabled_plugins().len(), 0);
        mgr.enable("test-plugin").unwrap();
        assert_eq!(mgr.enabled_plugins().len(), 1);
        mgr.disable("test-plugin").unwrap();
        assert_eq!(mgr.enabled_plugins().len(), 0);
    }

    #[test]
    fn enable_nonexistent() {
        let mut mgr = PluginManager::new();
        assert!(mgr.enable("ghost").is_err());
    }

    #[test]
    fn plugin_serialization() {
        let plugin = Plugin {
            name: "example".into(),
            description: "Example plugin".into(),
            enabled: true,
            mcp_servers: vec![McpServerConfig {
                name: "my-server".into(),
                command: "node".into(),
                args: vec!["server.js".into()],
            }],
        };
        let json = serde_json::to_string(&plugin).unwrap();
        let back: Plugin = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "example");
        assert_eq!(back.mcp_servers.len(), 1);
    }
}
