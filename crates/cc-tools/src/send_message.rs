//! SendMessageTool -- send a message to another agent or session.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct SendMessageTool;

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str {
        "send_message"
    }

    fn description(&self) -> &str {
        "Send a message to another agent or session"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "The recipient agent or session ID"
                },
                "content": {
                    "type": "string",
                    "description": "The message content to send"
                }
            },
            "required": ["to", "content"]
        })
    }

    async fn call(&self, input: serde_json::Value, _ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        let to = input.get("to").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("send_message", "Missing required field: to"))?;
        let content = input.get("content").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("send_message", "Missing required field: content"))?;

        // Message delivery is handled by the orchestrator layer.
        Ok(ToolOutput::success(format!(
            "Message queued for delivery to '{}' ({} chars)",
            to,
            content.len()
        )))
    }
}
