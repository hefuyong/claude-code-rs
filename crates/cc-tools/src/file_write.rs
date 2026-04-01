//! FileWriteTool -- create or overwrite files.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::path::Path;

pub struct FileWriteTool;

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }

    fn description(&self) -> &str {
        "Write content to a file, creating parent directories as needed"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"]
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
            .ok_or_else(|| CcError::tool("file_write", "Missing required field: file_path"))?;

        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("file_write", "Missing required field: content"))?;

        let file_path = Path::new(file_path_str);
        let resolved = if file_path.is_absolute() {
            file_path.to_path_buf()
        } else {
            ctx.working_directory.join(file_path)
        };

        if !ctx.permission_context.is_path_allowed(&resolved, true) {
            return Err(CcError::PermissionDenied(format!(
                "Write access denied for: {}",
                resolved.display()
            )));
        }

        // Create parent directories if they do not exist.
        if let Some(parent) = resolved.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| CcError::tool("file_write", format!("mkdir failed: {}", e)))?;
        }

        tokio::fs::write(&resolved, content)
            .await
            .map_err(|e| CcError::tool("file_write", format!("Write failed: {}", e)))?;

        let line_count = content.lines().count();
        Ok(ToolOutput::success(format!(
            "Wrote {} lines to {}",
            line_count,
            resolved.display()
        )))
    }
}
