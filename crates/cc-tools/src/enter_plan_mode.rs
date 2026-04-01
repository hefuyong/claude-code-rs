//! EnterPlanModeTool -- switch the session into planning mode.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct EnterPlanModeTool;

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        "enter_plan_mode"
    }

    fn description(&self) -> &str {
        "Enter planning mode to outline an approach before making changes"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn call(&self, _input: serde_json::Value, _ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        Ok(ToolOutput::success(
            "Entered planning mode. You can now outline your approach \
             without making changes. Use exit_plan_mode when ready to execute."
        ))
    }

    fn is_read_only(&self) -> bool {
        true
    }
}
