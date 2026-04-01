//! EnterWorktreeTool -- create a git worktree for isolated work.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use tokio::process::Command;

pub struct EnterWorktreeTool;

#[async_trait]
impl Tool for EnterWorktreeTool {
    fn name(&self) -> &str {
        "enter_worktree"
    }

    fn description(&self) -> &str {
        "Create an isolated git worktree and switch the session into it"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Optional name for the worktree (auto-generated if omitted)"
                }
            },
            "required": []
        })
    }

    async fn call(&self, input: serde_json::Value, ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        let name = input.get("name").and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("wt-{}", uuid_short()));

        let worktree_dir = ctx.working_directory.join(".claude").join("worktrees").join(&name);
        let branch_name = format!("worktree/{}", name);

        let output = Command::new("git")
            .args(["worktree", "add", "-b", &branch_name])
            .arg(worktree_dir.to_str().unwrap_or("."))
            .current_dir(&ctx.working_directory)
            .output().await
            .map_err(|e| CcError::tool("enter_worktree", format!("git worktree add failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(ToolOutput::error(format!("git worktree add failed: {}", stderr)));
        }

        Ok(ToolOutput::success(format!(
            "Created worktree '{}' at {} on branch '{}'",
            name, worktree_dir.display(), branch_name
        )))
    }
}

fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    format!("{:x}", ts & 0xFFFF_FFFF)
}
