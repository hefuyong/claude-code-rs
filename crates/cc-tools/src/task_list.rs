//! TaskListTool -- list all tasks in the task tracker.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct TaskListTool;

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        "task_list"
    }

    fn description(&self) -> &str {
        "List all tasks in the task tracker"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn call(&self, _input: serde_json::Value, _ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        // Task state is managed by the orchestrator; this tool requests
        // the current task list from it.  Placeholder returns an empty list.
        let result = serde_json::json!({
            "tasks": [],
            "summary": {
                "total": 0,
                "pending": 0,
                "in_progress": 0,
                "completed": 0
            }
        });

        Ok(ToolOutput::success(serde_json::to_string_pretty(&result).unwrap_or_default()))
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
}
