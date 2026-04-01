//! Model Context Protocol (MCP) client for Claude Code RS.
//!
//! Implements the MCP protocol for connecting to external tool servers
//! via stdio, SSE, or HTTP transports.
//!
//! # Architecture
//!
//! - [`protocol`] -- JSON-RPC 2.0 wire types
//! - [`types`] -- MCP domain objects (tools, resources, prompts, config)
//! - [`transport`] -- pluggable transport layer (stdio, SSE, HTTP)
//! - [`manager`] -- multi-server connection pool
//! - [`permissions`] -- access control for servers and tools
//!
//! The primary entry-point for callers is [`McpClient`] (single server)
//! or [`McpConnectionManager`] (multiple servers).

pub mod manager;
pub mod permissions;
pub mod protocol;
pub mod transport;
pub mod types;

// Re-export the most commonly used items at crate root.
pub use manager::McpConnectionManager;
pub use permissions::{McpPermissions, ToolPermission};
pub use protocol::{JsonRpcError, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
pub use transport::McpTransport;
pub use types::{
    McpPrompt, McpPromptArgument, McpResource, McpServerConfig, McpTool, ServerCapabilities,
    TransportConfig,
};

// ── McpClient ──────────────────────────────────────────────────────

/// The `client` module contains the [`McpClient`] implementation.
/// It is kept as a private module so that the struct is re-exported
/// directly from the crate root.
mod client {
    use cc_error::{CcError, CcResult};

    use crate::protocol::RequestIdGenerator;
    use crate::transport::{HttpStreamableTransport, McpTransport, SseTransport, StdioTransport};
    use crate::types::{
        McpPrompt, McpResource, McpServerConfig, McpTool, ServerCapabilities, TransportConfig,
    };

    /// High-level client for interacting with a single MCP server.
    ///
    /// Wraps a transport and exposes typed methods for every MCP
    /// operation (tools, resources, prompts).
    pub struct McpClient {
        transport: Box<dyn McpTransport>,
        #[allow(dead_code)]
        id_gen: RequestIdGenerator,
        capabilities: Option<ServerCapabilities>,
    }

    impl McpClient {
        /// Create a client connected to the server described by `config`.
        ///
        /// This opens the transport but does **not** send the MCP
        /// `initialize` handshake -- call [`initialize`](Self::initialize)
        /// next.
        pub async fn connect(config: &McpServerConfig) -> CcResult<Self> {
            let transport: Box<dyn McpTransport> = match &config.transport {
                TransportConfig::Stdio { command, args } => {
                    Box::new(StdioTransport::spawn(command, args, &config.env).await?)
                }
                TransportConfig::Sse { url } => Box::new(SseTransport::new(url.clone())),
                TransportConfig::Http { url } => {
                    Box::new(HttpStreamableTransport::new(url.clone()))
                }
            };

            Ok(Self {
                transport,
                id_gen: RequestIdGenerator::new(),
                capabilities: None,
            })
        }

        /// Perform the MCP `initialize` handshake.
        ///
        /// Sends the client capabilities and protocol version, receives
        /// the server capabilities, then sends `notifications/initialized`.
        pub async fn initialize(&mut self) -> CcResult<ServerCapabilities> {
            let init_params = serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "claude-code-rs",
                    "version": env!("CARGO_PKG_VERSION")
                }
            });

            let response = self
                .transport
                .send_request("initialize", Some(init_params))
                .await?;

            // Parse capabilities from the response.
            let result = response
                .get("result")
                .cloned()
                .unwrap_or(response.clone());
            let capabilities = ServerCapabilities::from_init_response(&result);

            // Send the initialized notification.
            let _ = self
                .transport
                .send_notification("notifications/initialized", None)
                .await;

            tracing::debug!(
                tools = capabilities.tools,
                resources = capabilities.resources,
                prompts = capabilities.prompts,
                "MCP session initialized"
            );

            self.capabilities = Some(capabilities.clone());
            Ok(capabilities)
        }

        // ── Tools ──────────────────────────────────────────────────

        /// List tools available on the server.
        pub async fn list_tools(&self) -> CcResult<Vec<McpTool>> {
            let response = self
                .transport
                .send_request("tools/list", Some(serde_json::json!({})))
                .await?;

            let tools_val = extract_field(&response, "tools");
            serde_json::from_value(tools_val)
                .map_err(|e| CcError::Serialization(format!("failed to parse MCP tools: {e}")))
        }

        /// Call a tool on the server.
        pub async fn call_tool(
            &self,
            name: &str,
            input: serde_json::Value,
        ) -> CcResult<serde_json::Value> {
            let params = serde_json::json!({
                "name": name,
                "arguments": input,
            });

            let response = self
                .transport
                .send_request("tools/call", Some(params))
                .await?;

            // Check for JSON-RPC error.
            if let Some(error) = response.get("error") {
                let msg = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown MCP error");
                return Err(CcError::Tool {
                    tool_name: name.to_string(),
                    message: msg.to_string(),
                });
            }

            Ok(response
                .get("result")
                .cloned()
                .unwrap_or(serde_json::Value::Null))
        }

        // ── Resources ──────────────────────────────────────────────

        /// List resources available on the server.
        pub async fn list_resources(&self) -> CcResult<Vec<McpResource>> {
            let response = self
                .transport
                .send_request("resources/list", Some(serde_json::json!({})))
                .await?;

            let resources_val = extract_field(&response, "resources");
            serde_json::from_value(resources_val).map_err(|e| {
                CcError::Serialization(format!("failed to parse MCP resources: {e}"))
            })
        }

        /// Read a resource by URI.
        pub async fn read_resource(&self, uri: &str) -> CcResult<String> {
            let params = serde_json::json!({ "uri": uri });
            let response = self
                .transport
                .send_request("resources/read", Some(params))
                .await?;

            let content = response
                .get("result")
                .and_then(|r| r.get("contents"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|item| item.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("");

            Ok(content.to_string())
        }

        // ── Prompts ────────────────────────────────────────────────

        /// List prompts available on the server.
        pub async fn list_prompts(&self) -> CcResult<Vec<McpPrompt>> {
            let response = self
                .transport
                .send_request("prompts/list", Some(serde_json::json!({})))
                .await?;

            let prompts_val = extract_field(&response, "prompts");
            serde_json::from_value(prompts_val)
                .map_err(|e| CcError::Serialization(format!("failed to parse MCP prompts: {e}")))
        }

        // ── Lifecycle ──────────────────────────────────────────────

        /// Close the connection to the server.
        pub async fn close(&self) -> CcResult<()> {
            self.transport.close().await
        }

        /// Returns the server capabilities obtained during initialization.
        pub fn capabilities(&self) -> Option<&ServerCapabilities> {
            self.capabilities.as_ref()
        }

        /// Returns `true` if the underlying transport is still connected.
        pub fn is_connected(&self) -> bool {
            self.transport.is_connected()
        }
    }

    /// Helper: extract a field from a JSON-RPC result, falling back to an
    /// empty array if missing.
    fn extract_field(response: &serde_json::Value, field: &str) -> serde_json::Value {
        response
            .get("result")
            .and_then(|r| r.get(field))
            .cloned()
            .unwrap_or(serde_json::Value::Array(vec![]))
    }
}

// Re-export McpClient at crate root.
pub use client::McpClient;

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_tool_deserialize() {
        let json = serde_json::json!({
            "name": "read_file",
            "description": "Read a file",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                }
            }
        });
        let tool: McpTool = serde_json::from_value(json).unwrap();
        assert_eq!(tool.name, "read_file");
    }

    #[test]
    fn mcp_resource_deserialize() {
        let json = serde_json::json!({
            "uri": "file:///tmp/test.txt",
            "name": "test.txt",
            "mime_type": "text/plain"
        });
        let res: McpResource = serde_json::from_value(json).unwrap();
        assert_eq!(res.uri, "file:///tmp/test.txt");
        assert_eq!(res.mime_type, Some("text/plain".to_string()));
    }

    #[test]
    fn server_config_serde_roundtrip() {
        let config = McpServerConfig {
            name: "test-server".into(),
            transport: TransportConfig::Stdio {
                command: "node".into(),
                args: vec!["server.js".into()],
            },
            env: Default::default(),
        };
        let json = serde_json::to_value(&config).unwrap();
        let parsed: McpServerConfig = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.name, "test-server");
    }

    #[test]
    fn connection_manager_starts_empty() {
        let mgr = McpConnectionManager::new();
        assert!(mgr.list_connections().is_empty());
        assert!(mgr.get_all_tools().is_empty());
        assert!(mgr.get_all_resources().is_empty());
    }

    #[test]
    fn test_protocol_request_serialization() {
        let req = JsonRpcRequest::new(42, "tools/call", Some(serde_json::json!({"name": "bash"})));
        let json_str = serde_json::to_string(&req).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["id"], 42);
        assert_eq!(value["method"], "tools/call");
        assert_eq!(value["params"]["name"], "bash");

        // Verify that a request without params omits the field.
        let req_no_params = JsonRpcRequest::new(1, "resources/list", None);
        let json_no_params = serde_json::to_string(&req_no_params).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json_no_params).unwrap();
        assert!(val.get("params").is_none());
    }

    #[test]
    fn test_protocol_response_parsing() {
        // Parse a successful response.
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"tools":[]}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, 1);
        assert!(!resp.is_error());
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());

        // Parse an error response.
        let err_json = r#"{"jsonrpc":"2.0","id":2,"error":{"code":-32601,"message":"Method not found"}}"#;
        let err_resp: JsonRpcResponse = serde_json::from_str(err_json).unwrap();
        assert_eq!(err_resp.id, 2);
        assert!(err_resp.is_error());
        assert!(err_resp.result.is_none());
        let err = err_resp.error.unwrap();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "Method not found");
    }

    #[test]
    fn test_manager_no_connections() {
        let mgr = McpConnectionManager::new();
        assert!(mgr.list_connections().is_empty());
        assert!(mgr.get_all_tools().is_empty());
        assert!(mgr.get_all_resources().is_empty());
        assert!(mgr.get_capabilities("nonexistent").is_none());
    }

    #[test]
    fn test_permissions_allow_all() {
        let perms = McpPermissions::allow_all();
        // An allow-all config should let any server through.
        assert!(perms.check_server("any-server"));
        assert!(perms.check_server("another-server"));
        assert!(perms.check_server(""));
        // Tools should default to Ask when no explicit rule exists.
        assert_eq!(
            perms.check_tool("any-server", "any-tool"),
            ToolPermission::Ask
        );
    }

    #[test]
    fn test_permissions_deny_server() {
        let perms = McpPermissions {
            denied_servers: vec!["blocked-server".into()],
            ..Default::default()
        };
        assert!(!perms.check_server("blocked-server"));
        assert!(perms.check_server("allowed-server"));

        // Deny takes precedence even if also in allowed list.
        let perms2 = McpPermissions {
            allowed_servers: vec!["dual".into()],
            denied_servers: vec!["dual".into()],
            ..Default::default()
        };
        assert!(!perms2.check_server("dual"));
    }
}
