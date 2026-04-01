//! PowerShellTool -- execute PowerShell commands on Windows.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::time::Duration;
use tokio::process::Command;

pub struct PowerShellTool;

#[async_trait]
impl Tool for PowerShellTool {
    fn name(&self) -> &str {
        "powershell"
    }

    fn description(&self) -> &str {
        "Execute a PowerShell command on Windows and return its output"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The PowerShell command to execute"
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
            .ok_or_else(|| CcError::tool("powershell", "Missing required field: command"))?;

        let timeout_ms = input
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(120_000);

        // Locate the PowerShell executable.
        let ps_exe = if cfg!(windows) {
            "powershell.exe"
        } else {
            // pwsh is the cross-platform PowerShell Core binary.
            "pwsh"
        };

        let child = Command::new(ps_exe)
            .args(["-NoProfile", "-NonInteractive", "-Command", command])
            .current_dir(&ctx.working_directory)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                CcError::tool("powershell", format!("Failed to spawn {}: {}", ps_exe, e))
            })?;

        let result =
            tokio::time::timeout(Duration::from_millis(timeout_ms), child.wait_with_output())
                .await
                .map_err(|_| {
                    CcError::tool(
                        "powershell",
                        format!("Command timed out after {}ms", timeout_ms),
                    )
                })?
                .map_err(|e| CcError::tool("powershell", format!("Process error: {}", e)))?;

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

    fn is_read_only(&self) -> bool {
        false
    }
}
