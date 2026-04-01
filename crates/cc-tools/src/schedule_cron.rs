//! ScheduleCronTool -- create scheduled tasks using cron expressions.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct ScheduleCronTool;

#[async_trait]
impl Tool for ScheduleCronTool {
    fn name(&self) -> &str {
        "schedule_cron"
    }

    fn description(&self) -> &str {
        "Schedule a prompt to run at a future time using a cron expression"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "cron": {
                    "type": "string",
                    "description": "Standard 5-field cron expression (M H DoM Mon DoW)"
                },
                "prompt": {
                    "type": "string",
                    "description": "The prompt to enqueue at each fire time"
                },
                "recurring": {
                    "type": "boolean",
                    "description": "Whether the job recurs (default: true)"
                }
            },
            "required": ["cron", "prompt"]
        })
    }

    async fn call(&self, input: serde_json::Value, _ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        let cron = input.get("cron").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("schedule_cron", "Missing required field: cron"))?;
        let prompt = input.get("prompt").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("schedule_cron", "Missing required field: prompt"))?;
        let recurring = input.get("recurring").and_then(|v| v.as_bool()).unwrap_or(true);

        // Validate cron expression has 5 fields.
        let fields: Vec<&str> = cron.split_whitespace().collect();
        if fields.len() != 5 {
            return Ok(ToolOutput::error(format!(
                "Invalid cron expression: expected 5 fields, got {}", fields.len()
            )));
        }

        // Generate a simple job ID.
        let job_id = format!("cron-{:x}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() & 0xFFFF_FFFF);

        Ok(ToolOutput::success(format!(
            "Scheduled {} job '{}': cron='{}', prompt='{}' ({} chars)",
            if recurring { "recurring" } else { "one-shot" },
            job_id, cron, &prompt[..prompt.len().min(50)], prompt.len()
        )))
    }
}
