//! TaskGetTool -- retrieve details for a specific task.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct TaskGetTool;

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        "task_get"
    }

    fn description(&self) -> &str {
        "Get the full details of a task by its ID"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "taskId": {
                    "type": "string",
                    "description": "The ID of the task to retrieve"
                }
            },
            "required": ["taskId"]
        })
    }

    async fn call(&self, input: serde_json::Value, _ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        let task_id = input.get("taskId").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("task_get", "Missing required field: taskId"))?;

        // Task lookup is handled by the orchestrator.
        // Return a placeholder indicating the task was not found.
        Ok(ToolOutput::error(format!("Task '{}' not found in current session", task_id)))
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
}
