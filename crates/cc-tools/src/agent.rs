//! AgentTool -- spawn a sub-agent as a subprocess.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::time::Duration;
use tokio::process::Command;

pub struct AgentTool;

#[async_trait]
impl Tool for AgentTool {
    fn name(&self) -> &str {
        "agent"
    }

    fn description(&self) -> &str {
        "Spawn a sub-agent to handle a complex task independently"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The prompt to send to the sub-agent"
                },
                "description": {
                    "type": "string",
                    "description": "A brief description of the task for the sub-agent"
                },
                "subagent_type": {
                    "type": "string",
                    "description": "Type of sub-agent to spawn (default: 'default')"
                },
                "model": {
                    "type": "string",
                    "description": "Model to use for the sub-agent"
                }
            },
            "required": ["prompt", "description"]
        })
    }

    async fn call(&self, input: serde_json::Value, ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        let prompt = input.get("prompt").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("agent", "Missing required field: prompt"))?;

        let _description = input.get("description").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("agent", "Missing required field: description"))?;

        let model = input.get("model").and_then(|v| v.as_str()).unwrap_or("default");

        let mut cmd = Command::new("claude-code");
        cmd.arg("--print").arg(prompt).arg("--model").arg(model)
            .current_dir(&ctx.working_directory)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let child = cmd.spawn()
            .map_err(|e| CcError::tool("agent", format!("Failed to spawn sub-agent: {}", e)))?;

        let result = tokio::time::timeout(Duration::from_secs(300), child.wait_with_output())
            .await
            .map_err(|_| CcError::tool("agent", "Sub-agent timed out after 300s"))?
            .map_err(|e| CcError::tool("agent", format!("Sub-agent error: {}", e)))?;

        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);

        let mut output = stdout.to_string();
        if !stderr.is_empty() {
            output.push_str("\n[stderr] ");
            output.push_str(&stderr);
        }

        Ok(ToolOutput {
            content: output,
            is_error: !result.status.success(),
        })
    }
}
