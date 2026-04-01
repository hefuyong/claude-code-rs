//! TaskUpdateTool -- update an existing task's status or details.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct TaskUpdateTool;

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        "task_update"
    }

    fn description(&self) -> &str {
        "Update an existing task's status, subject, or description"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "taskId": {
                    "type": "string",
                    "description": "The ID of the task to update"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed", "deleted"],
                    "description": "New status for the task"
                },
                "subject": {
                    "type": "string",
                    "description": "New subject/title for the task"
                },
                "description": {
                    "type": "string",
                    "description": "New description for the task"
                }
            },
            "required": ["taskId"]
        })
    }

    async fn call(&self, input: serde_json::Value, _ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        let task_id = input.get("taskId").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("task_update", "Missing required field: taskId"))?;
        let status = input.get("status").and_then(|v| v.as_str());
        let subject = input.get("subject").and_then(|v| v.as_str());
        let description = input.get("description").and_then(|v| v.as_str());

        let mut updates = Vec::new();
        if let Some(s) = status { updates.push(format!("status={}", s)); }
        if let Some(s) = subject { updates.push(format!("subject={}", s)); }
        if let Some(s) = description { updates.push(format!("description={}...", &s[..s.len().min(30)])); }

        if updates.is_empty() {
            return Ok(ToolOutput::error("No fields to update"));
        }

        Ok(ToolOutput::success(format!(
            "Task '{}' updated: {}", task_id, updates.join(", ")
        )))
    }
}
