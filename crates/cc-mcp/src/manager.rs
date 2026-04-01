//! Connection manager for multiple MCP servers.
//!
//! [`McpConnectionManager`] maintains a pool of named connections,
//! each backed by an [`McpClient`].  It provides a single entry-point
//! for listing tools/resources across all connected servers and for
//! dispatching tool calls and resource reads to the correct server.

use cc_error::{CcError, CcResult};
use std::collections::HashMap;

use crate::client::McpClient;
use crate::types::{McpResource, McpServerConfig, McpTool, ServerCapabilities};

/// A single live connection to an MCP server.
struct McpConnection {
    #[allow(dead_code)]
    config: McpServerConfig,
    client: McpClient,
    capabilities: ServerCapabilities,
    tools: Vec<McpTool>,
    resources: Vec<McpResource>,
}

/// Manages connections to multiple MCP servers.
///
/// # Example
///
/// ```rust,ignore
/// let mut mgr = McpConnectionManager::new();
/// mgr.connect(config).await?;
/// let tools = mgr.get_all_tools();
/// let result = mgr.call_tool("my-server", "read_file", json!({"path": "/tmp/a"})).await?;
/// mgr.disconnect_all().await?;
/// ```
pub struct McpConnectionManager {
    connections: HashMap<String, McpConnection>,
}

impl McpConnectionManager {
    /// Create an empty manager with no connections.
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    /// Connect to an MCP server described by `config`.
    ///
    /// Performs the initialize handshake and fetches the server's tool
    /// and resource lists.  The connection is stored under
    /// `config.name`.
    pub async fn connect(&mut self, config: McpServerConfig) -> CcResult<()> {
        let name = config.name.clone();
        tracing::info!(server = %name, "connecting to MCP server");

        let mut client = McpClient::connect(&config).await?;
        let capabilities = client.initialize().await?;

        let tools = if capabilities.tools {
            client.list_tools().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        let resources = if capabilities.resources {
            client.list_resources().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        tracing::info!(
            server = %name,
            tools = tools.len(),
            resources = resources.len(),
            "MCP server connected"
        );

        self.connections.insert(
            name,
            McpConnection {
                config,
                client,
                capabilities,
                tools,
                resources,
            },
        );

        Ok(())
    }

    /// Disconnect from a named server.
    pub async fn disconnect(&mut self, name: &str) -> CcResult<()> {
        if let Some(conn) = self.connections.remove(name) {
            conn.client.close().await?;
            tracing::info!(server = %name, "disconnected from MCP server");
            Ok(())
        } else {
            Err(CcError::NotFound(format!(
                "MCP server '{name}' is not connected"
            )))
        }
    }

    /// Disconnect from all servers.
    pub async fn disconnect_all(&mut self) -> CcResult<()> {
        let names: Vec<String> = self.connections.keys().cloned().collect();
        for name in names {
            if let Some(conn) = self.connections.remove(&name) {
                let _ = conn.client.close().await;
            }
        }
        Ok(())
    }

    /// List the names of all currently connected servers.
    pub fn list_connections(&self) -> Vec<&str> {
        self.connections.keys().map(|s| s.as_str()).collect()
    }

    /// Get all tools across every connected server.
    ///
    /// Returns `(server_name, tool)` pairs.
    pub fn get_all_tools(&self) -> Vec<(&str, &McpTool)> {
        self.connections
            .iter()
            .flat_map(|(name, conn)| conn.tools.iter().map(move |t| (name.as_str(), t)))
            .collect()
    }

    /// Get all resources across every connected server.
    ///
    /// Returns `(server_name, resource)` pairs.
    pub fn get_all_resources(&self) -> Vec<(&str, &McpResource)> {
        self.connections
            .iter()
            .flat_map(|(name, conn)| conn.resources.iter().map(move |r| (name.as_str(), r)))
            .collect()
    }

    /// Get the capabilities reported by a named server.
    pub fn get_capabilities(&self, name: &str) -> Option<&ServerCapabilities> {
        self.connections.get(name).map(|c| &c.capabilities)
    }

    /// Call a tool on a specific server.
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        input: serde_json::Value,
    ) -> CcResult<serde_json::Value> {
        let conn = self.connections.get(server).ok_or_else(|| {
            CcError::NotFound(format!("MCP server '{server}' is not connected"))
        })?;
        conn.client.call_tool(tool, input).await
    }

    /// Read a resource from a specific server.
    pub async fn read_resource(&self, server: &str, uri: &str) -> CcResult<String> {
        let conn = self.connections.get(server).ok_or_else(|| {
            CcError::NotFound(format!("MCP server '{server}' is not connected"))
        })?;
        conn.client.read_resource(uri).await
    }
}

impl Default for McpConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}
