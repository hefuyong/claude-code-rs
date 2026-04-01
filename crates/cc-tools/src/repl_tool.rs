//! REPLTool -- run code snippets in a subprocess REPL (python, node, bash).

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::time::Duration;
use tokio::process::Command;

pub struct REPLTool;

#[async_trait]
impl Tool for REPLTool {
    fn name(&self) -> &str {
        "repl"
    }

    fn description(&self) -> &str {
        "Run code in a REPL subprocess (python, node, or bash) and return the output"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "language": {
                    "type": "string",
                    "enum": ["python", "node", "bash"],
                    "description": "The language REPL to use"
                },
                "code": {
                    "type": "string",
                    "description": "The code to execute"
                }
            },
            "required": ["language", "code"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let language = input
            .get("language")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("repl", "Missing required field: language"))?;

        let code = input
            .get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("repl", "Missing required field: code"))?;

        let (program, args): (&str, Vec<&str>) = match language {
            "python" => ("python3", vec!["-c", code]),
            "node" => ("node", vec!["-e", code]),
            "bash" => ("bash", vec!["-c", code]),
            other => {
                return Err(CcError::tool(
                    "repl",
                    format!("Unsupported language: {}. Use python, node, or bash.", other),
                ));
            }
        };

        let child = Command::new(program)
            .args(&args)
            .current_dir(&ctx.working_directory)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| CcError::tool("repl", format!("Failed to spawn {}: {}", program, e)))?;

        let result = tokio::time::timeout(Duration::from_secs(60), child.wait_with_output())
            .await
            .map_err(|_| CcError::tool("repl", "REPL execution timed out after 60s"))?
            .map_err(|e| CcError::tool("repl", format!("Process error: {}", e)))?;

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
        Ok(ToolOutput {
            content: output,
            is_error: exit_code != 0,
        })
    }

    fn is_read_only(&self) -> bool {
        false
    }
}
