//! SleepTool -- pause execution for a specified duration.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::time::Duration;

pub struct SleepTool;

/// Maximum allowed sleep duration (5 minutes).
const MAX_SLEEP_MS: u64 = 300_000;

#[async_trait]
impl Tool for SleepTool {
    fn name(&self) -> &str {
        "sleep"
    }

    fn description(&self) -> &str {
        "Pause execution for the specified number of milliseconds"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "duration_ms": {
                    "type": "number",
                    "description": "Duration to sleep in milliseconds (max 300000)"
                }
            },
            "required": ["duration_ms"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let duration_ms = input
            .get("duration_ms")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| CcError::tool("sleep", "Missing required field: duration_ms"))?;

        if duration_ms > MAX_SLEEP_MS {
            return Err(CcError::tool(
                "sleep",
                format!("Duration {}ms exceeds maximum of {}ms", duration_ms, MAX_SLEEP_MS),
            ));
        }

        tokio::time::sleep(Duration::from_millis(duration_ms)).await;

        Ok(ToolOutput::success(format!(
            "Slept for {}ms.",
            duration_ms
        )))
    }

    fn is_read_only(&self) -> bool {
        true
    }
}
