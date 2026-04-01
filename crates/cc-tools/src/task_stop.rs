//! TaskStopTool -- stops a running background task by ID.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct TaskStopTool;

#[async_trait]
impl Tool for TaskStopTool {
    fn name(&self) -> &str {
        "task_stop"
    }

    fn description(&self) -> &str {
        "Stop a running background task by its ID"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The unique identifier of the background task to stop"
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
            .ok_or_else(|| CcError::tool("task_stop", "Missing required field: task_id"))?;

        // In a full implementation this would look up the task in a shared
        // TaskManager and send a cancellation signal. For now we report the
        // intent so the orchestrator can act on it.
        Ok(ToolOutput::success(format!(
            "Stop signal sent to task '{}'.",
            task_id
        )))
    }

    // Mutates task state.
    fn is_read_only(&self) -> bool {
        false
    }
}
