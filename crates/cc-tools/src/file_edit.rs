//! FileEditTool -- find-and-replace in files.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::path::Path;

pub struct FileEditTool;

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "file_edit"
    }

    fn description(&self) -> &str {
        "Perform an exact string replacement in a file"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to find and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement string"
                }
            },
            "required": ["file_path", "old_string", "new_string"]
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
            .ok_or_else(|| CcError::tool("file_edit", "Missing required field: file_path"))?;

        let old_string = input
            .get("old_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("file_edit", "Missing required field: old_string"))?;

        let new_string = input
            .get("new_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("file_edit", "Missing required field: new_string"))?;

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

        let content = tokio::fs::read_to_string(&resolved)
            .await
            .map_err(|e| CcError::tool("file_edit", format!("{}: {}", resolved.display(), e)))?;

        // Ensure old_string exists and is unique.
        let count = content.matches(old_string).count();
        if count == 0 {
            return Ok(ToolOutput::error(format!(
                "old_string not found in {}",
                resolved.display()
            )));
        }
        if count > 1 {
            return Ok(ToolOutput::error(format!(
                "old_string found {} times in {} -- it must be unique. \
                 Provide more surrounding context to disambiguate.",
                count,
                resolved.display()
            )));
        }

        let new_content = content.replacen(old_string, new_string, 1);
        tokio::fs::write(&resolved, &new_content)
            .await
            .map_err(|e| CcError::tool("file_edit", format!("Write failed: {}", e)))?;

        Ok(ToolOutput::success(format!(
            "Successfully edited {}",
            resolved.display()
        )))
    }
}
