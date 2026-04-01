//! ExitPlanModeTool -- leave planning mode and resume execution.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct ExitPlanModeTool;

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "exit_plan_mode"
    }

    fn description(&self) -> &str {
        "Exit planning mode and resume normal tool execution"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "allowedPrompts": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional list of prompts allowed after exiting plan mode"
                }
            },
            "required": []
        })
    }

    async fn call(&self, _input: serde_json::Value, _ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        Ok(ToolOutput::success(
            "Exited planning mode. You may now execute tools and make changes."
        ))
    }

    fn is_read_only(&self) -> bool {
        true
    }
}
