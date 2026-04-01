//! ReadMcpResourceTool -- read a specific resource from an MCP server.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct ReadMcpResourceTool;

#[async_trait]
impl Tool for ReadMcpResourceTool {
    fn name(&self) -> &str {
        "read_mcp_resource"
    }

    fn description(&self) -> &str {
        "Read the contents of a resource from a connected MCP server by URI"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "server_name": {
                    "type": "string",
                    "description": "Name of the MCP server that owns the resource"
                },
                "uri": {
                    "type": "string",
                    "description": "URI of the resource to read"
                }
            },
            "required": ["server_name", "uri"]
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
            .ok_or_else(|| {
                CcError::tool("read_mcp_resource", "Missing required field: server_name")
            })?;

        let uri = input
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CcError::tool("read_mcp_resource", "Missing required field: uri")
            })?;

        // Placeholder: in production this would forward to the MCP client for
        // the named server and read the resource at the given URI.
        Ok(ToolOutput::error(format!(
            "MCP server '{}' is not connected. Cannot read resource '{}'.",
            server_name, uri
        )))
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
}
