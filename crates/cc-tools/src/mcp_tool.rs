//! MCPTool -- forward a tool call to an MCP (Model Context Protocol) server.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct MCPTool;

#[async_trait]
impl Tool for MCPTool {
    fn name(&self) -> &str {
        "mcp_tool"
    }

    fn description(&self) -> &str {
        "Call a tool exposed by a connected MCP server"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "server_name": {
                    "type": "string",
                    "description": "Name of the MCP server to call"
                },
                "tool_name": {
                    "type": "string",
                    "description": "Name of the tool on the MCP server"
                },
                "arguments": {
                    "type": "object",
                    "description": "Arguments to pass to the MCP tool"
                }
            },
            "required": ["server_name", "tool_name", "arguments"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let server_name = input
            .get("server_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("mcp_tool", "Missing required field: server_name"))?;

        let tool_name = input
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("mcp_tool", "Missing required field: tool_name"))?;

        let arguments = input
            .get("arguments")
            .ok_or_else(|| CcError::tool("mcp_tool", "Missing required field: arguments"))?;

        // Placeholder: in production this would look up the MCP client for
        // `server_name` and forward the call. For now, return an informational
        // error so the caller knows the server is not connected.
        Ok(ToolOutput::error(format!(
            "MCP server '{}' is not connected. Cannot call tool '{}' with arguments: {}",
            server_name,
            tool_name,
            serde_json::to_string(arguments).unwrap_or_default()
        )))
    }

    // MCP tools may mutate state on the remote server.
    fn is_read_only(&self) -> bool {
        false
    }
}
