//! ExitWorktreeTool -- remove or keep a git worktree and return to the main tree.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use tokio::process::Command;

pub struct ExitWorktreeTool;

#[async_trait]
impl Tool for ExitWorktreeTool {
    fn name(&self) -> &str {
        "exit_worktree"
    }

    fn description(&self) -> &str {
        "Exit a worktree session, optionally removing it"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["keep", "remove"],
                    "description": "Whether to keep or remove the worktree"
                },
                "discard_changes": {
                    "type": "boolean",
                    "description": "Force removal even with uncommitted changes"
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, input: serde_json::Value, ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        let action = input.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("exit_worktree", "Missing required field: action"))?;
        let discard = input.get("discard_changes").and_then(|v| v.as_bool()).unwrap_or(false);

        match action {
            "keep" => {
                Ok(ToolOutput::success("Worktree kept. Returning to main working directory."))
            }
            "remove" => {
                let mut args = vec!["worktree", "remove"];
                if discard { args.push("--force"); }
                let wt_path = ctx.working_directory.to_str().unwrap_or(".");
                args.push(wt_path);

                let output = Command::new("git").args(&args)
                    .output().await
                    .map_err(|e| CcError::tool("exit_worktree", format!("git worktree remove failed: {}", e)))?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Ok(ToolOutput::error(format!("Failed to remove worktree: {}", stderr)));
                }
                Ok(ToolOutput::success("Worktree removed. Returning to main working directory."))
            }
            _ => Ok(ToolOutput::error("action must be 'keep' or 'remove'")),
        }
    }
}
