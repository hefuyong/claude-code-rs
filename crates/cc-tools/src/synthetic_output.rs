//! SyntheticOutputTool -- generate a synthetic tool output for internal use.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct SyntheticOutputTool;

#[async_trait]
impl Tool for SyntheticOutputTool {
    fn name(&self) -> &str {
        "synthetic_output"
    }

    fn description(&self) -> &str {
        "Generate a synthetic tool output for internal orchestration purposes"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The content to include in the synthetic output"
                },
                "is_error": {
                    "type": "boolean",
                    "description": "Whether the output should be marked as an error (default: false)"
                }
            },
            "required": ["content"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CcError::tool("synthetic_output", "Missing required field: content")
            })?;

        let is_error = input
            .get("is_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(ToolOutput {
            content: content.to_string(),
            is_error,
        })
    }

    fn is_read_only(&self) -> bool {
        true
    }
}
