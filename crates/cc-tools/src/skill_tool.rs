//! SkillTool -- invoke a registered skill by name.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct SkillTool;

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "skill"
    }

    fn description(&self) -> &str {
        "Invoke a registered skill within the current conversation"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "skill": {
                    "type": "string",
                    "description": "The name of the skill to invoke"
                },
                "args": {
                    "type": "string",
                    "description": "Optional arguments for the skill"
                }
            },
            "required": ["skill"]
        })
    }

    async fn call(&self, input: serde_json::Value, _ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        let skill_name = input.get("skill").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("skill", "Missing required field: skill"))?;
        let args = input.get("args").and_then(|v| v.as_str()).unwrap_or("");

        // Skills are loaded dynamically from the skill registry.
        // This is a placeholder that returns the skill invocation request.
        Ok(ToolOutput::success(format!(
            "Invoking skill '{}' with args: {}. \
             Skill execution is handled by the orchestrator.",
            skill_name,
            if args.is_empty() { "(none)" } else { args }
        )))
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
}
