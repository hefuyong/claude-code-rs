//! FileReadTool -- read file contents with optional offset and line limit.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::path::Path;

pub struct FileReadTool;

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file, optionally with a line offset and limit"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to read"
                },
                "offset": {
                    "type": "number",
                    "description": "Line number to start reading from (1-based)"
                },
                "limit": {
                    "type": "number",
                    "description": "Maximum number of lines to read (default: 2000)"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let file_path_str = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("file_read", "Missing required field: file_path"))?;

        let offset = input
            .get("offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;
        let offset = if offset == 0 { 1 } else { offset };

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(2000) as usize;

        // Resolve relative paths against the working directory.
        let file_path = Path::new(file_path_str);
        let resolved = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            ctx.working_directory.join(file_path)
        };

        // Check permission.
        if !ctx.permission_context.is_path_allowed(&resolved, false) {
            return Err(CcError::PermissionDenied(format!(
                "Read access denied for: {}",
                resolved.display()
            )));
        }

        let content = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| CcError::tool("file_read", format!("{}: {}", resolved.display(), e)))?;

        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Apply offset (1-based) and limit.
        let start = (offset - 1).min(total_lines);
        let end = (start + limit).min(total_lines);

        let mut output = String::new();
        for (i, line) in lines[start..end].iter().enumerate() {
            let line_num = start + i + 1;
            output.push_str(&format!("{:>6}\t{}\n", line_num, line));
        }

        if end < total_lines {
            output.push_str(&format!(
                "\n... ({} more lines, {} total)\n",
                total_lines - end,
                total_lines
            ));
        }

        Ok(ToolOutput::success(output))
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
}
