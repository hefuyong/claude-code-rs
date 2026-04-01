//! BashTool -- execute shell commands via `tokio::process`.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::time::Duration;
use tokio::process::Command;

pub struct BashTool;

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a shell command and return its output"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": "Timeout in milliseconds (default: 120000)"
                }
            },
            "required": ["command"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("bash", "Missing required field: command"))?;

        let timeout_ms = input
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(120_000);

        let shell = if cfg!(windows) { "cmd" } else { "bash" };
        let flag = if cfg!(windows) { "/C" } else { "-c" };

        let child = Command::new(shell)
            .arg(flag)
            .arg(command)
            .current_dir(&ctx.working_directory)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| CcError::tool("bash", format!("Failed to spawn process: {}", e)))?;

        let result = tokio::time::timeout(Duration::from_millis(timeout_ms), child.wait_with_output())
            .await
            .map_err(|_| {
                CcError::tool(
                    "bash",
                    format!("Command timed out after {}ms", timeout_ms),
                )
            })?
            .map_err(|e| CcError::tool("bash", format!("Process error: {}", e)))?;

        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);

        let mut output = String::new();
        if !stdout.is_empty() {
            output.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&stderr);
        }

        let exit_code = result.status.code().unwrap_or(-1);
        if exit_code != 0 {
            output.push_str(&format!("\nExit code: {}", exit_code));
        }

        Ok(ToolOutput {
            content: output,
            is_error: exit_code != 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_permissions::{PermissionContext, PermissionMode};
    use tempfile::TempDir;

    fn make_ctx(dir: &TempDir) -> ToolContext {
        ToolContext {
            working_directory: dir.path().to_path_buf(),
            permission_context: PermissionContext::new(PermissionMode::Bypass, vec![]),
        }
    }

    #[tokio::test]
    async fn test_bash_echo() {
        let tmp = TempDir::new().unwrap();
        let tool = BashTool;
        let ctx = make_ctx(&tmp);

        let cmd = if cfg!(windows) { "echo hello" } else { "echo hello" };
        let input = serde_json::json!({ "command": cmd });

        let output = tool.call(input, &ctx).await.unwrap();
        assert!(!output.is_error);
        assert!(output.content.trim().contains("hello"));
    }

    #[tokio::test]
    async fn test_bash_exit_code() {
        let tmp = TempDir::new().unwrap();
        let tool = BashTool;
        let ctx = make_ctx(&tmp);

        let cmd = if cfg!(windows) { "exit 42" } else { "exit 42" };
        let input = serde_json::json!({ "command": cmd });

        let output = tool.call(input, &ctx).await.unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("Exit code:"));
    }

    #[tokio::test]
    async fn test_bash_timeout() {
        let tmp = TempDir::new().unwrap();
        let tool = BashTool;
        let ctx = make_ctx(&tmp);

        // Use a command that sleeps longer than the timeout.
        let cmd = if cfg!(windows) {
            "ping -n 10 127.0.0.1"
        } else {
            "sleep 10"
        };
        let input = serde_json::json!({
            "command": cmd,
            "timeout": 500
        });

        let result = tool.call(input, &ctx).await;
        // Should fail with a timeout error.
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("timed out"));
    }
}
