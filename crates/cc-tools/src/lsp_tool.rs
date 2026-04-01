//! LSPTool -- Language Server Protocol operations (placeholder).

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct LSPTool;

#[async_trait]
impl Tool for LSPTool {
    fn name(&self) -> &str {
        "lsp"
    }

    fn description(&self) -> &str {
        "Perform Language Server Protocol operations such as go-to-definition, hover, and references"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["definition", "hover", "references", "diagnostics", "completion"],
                    "description": "The LSP action to perform"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to the file (required for most actions)"
                },
                "position": {
                    "type": "object",
                    "description": "Cursor position in the file",
                    "properties": {
                        "line": { "type": "number", "description": "0-based line number" },
                        "character": { "type": "number", "description": "0-based character offset" }
                    }
                }
            },
            "required": ["action"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("lsp", "Missing required field: action"))?;

        let file_path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("<none>");

        // Placeholder -- no LSP client is wired up yet.
        Ok(ToolOutput::error(format!(
            "LSP not connected. Cannot perform '{}' on '{}'. \
             Connect a language server first.",
            action, file_path
        )))
    }

    fn is_read_only(&self) -> bool {
        true
    }
}
