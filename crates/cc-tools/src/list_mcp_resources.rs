//! ListMcpResourcesTool -- list resources exposed by MCP servers.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct ListMcpResourcesTool;

#[async_trait]
impl Tool for ListMcpResourcesTool {
    fn name(&self) -> &str {
        "list_mcp_resources"
    }

    fn description(&self) -> &str {
        "List resources available on connected MCP servers, optionally filtered by server name"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "server_name": {
                    "type": "string",
                    "description": "Optional MCP server name to filter by"
                }
            }
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let server_name = input
            .get("server_name")
            .and_then(|v| v.as_str());

        // Placeholder: in production this would query the MCP client registry
        // for all connected servers (or a specific one) and list their
        // advertised resources.
        match server_name {
            Some(name) => Ok(ToolOutput::success(format!(
                "No resources found. MCP server '{}' is not connected.",
                name
            ))),
            None => Ok(ToolOutput::success(
                "No MCP servers are currently connected. No resources available.",
            )),
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
}
