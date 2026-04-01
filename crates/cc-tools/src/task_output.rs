//! TaskOutputTool -- retrieves the output of a background task.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct TaskOutputTool;

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str {
        "task_output"
    }

    fn description(&self) -> &str {
        "Get the output of a background task, optionally blocking until output is available"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The unique identifier of the background task"
                },
                "block": {
                    "type": "boolean",
                    "description": "Whether to block until output is available (default: false)"
                },
                "timeout": {
                    "type": "number",
                    "description": "Maximum time to wait in milliseconds when blocking (default: 30000)"
                }
            },
            "required": ["task_id"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let task_id = input
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("task_output", "Missing required field: task_id"))?;

        let block = input
            .get("block")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let timeout_ms = input
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(30_000);

        // In a full implementation this would query a TaskManager for buffered
        // output, optionally waiting up to `timeout_ms` when `block` is true.
        if block {
            let _ = tokio::time::sleep(std::time::Duration::from_millis(
                timeout_ms.min(100),
            ))
            .await;
        }

        Ok(ToolOutput::success(format!(
            "No output available for task '{}'. (block={}, timeout={}ms)",
            task_id, block, timeout_ms
        )))
    }

    fn is_read_only(&self) -> bool {
        true
    }
}
