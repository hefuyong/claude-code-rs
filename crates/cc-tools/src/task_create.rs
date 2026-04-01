//! TaskCreateTool -- create a new task in the task tracker.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct TaskCreateTool;

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        "task_create"
    }

    fn description(&self) -> &str {
        "Create a new task to track progress on a piece of work"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "subject": {
                    "type": "string",
                    "description": "A brief title for the task"
                },
                "description": {
                    "type": "string",
                    "description": "Detailed description of what needs to be done"
                },
                "activeForm": {
                    "type": "string",
                    "description": "Present continuous form for spinner display (e.g. 'Running tests')"
                }
            },
            "required": ["subject", "description"]
        })
    }

    async fn call(&self, input: serde_json::Value, _ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        let subject = input.get("subject").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("task_create", "Missing required field: subject"))?;
        let description = input.get("description").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("task_create", "Missing required field: description"))?;
        let active_form = input.get("activeForm").and_then(|v| v.as_str());

        // Generate a simple task ID.
        let task_id = format!("{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() % 100_000);

        let result = serde_json::json!({
            "id": task_id,
            "subject": subject,
            "description": description,
            "activeForm": active_form.unwrap_or(subject),
            "status": "pending"
        });

        Ok(ToolOutput::success(serde_json::to_string_pretty(&result).unwrap_or_default()))
    }
}
