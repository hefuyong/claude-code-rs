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
    async fn test_write_new_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("output.txt");

        let tool = FileWriteTool;
        let ctx = make_ctx(&tmp);
        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "hello world\nsecond line"
        });

        let output = tool.call(input, &ctx).await.unwrap();
        assert!(!output.is_error);
        assert!(output.content.contains("Wrote 2 lines"));

        // Verify the file was actually created with the correct content.
        let written = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, "hello world\nsecond line");
    }

    #[tokio::test]
    async fn test_write_creates_directories() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("a").join("b").join("c").join("deep.txt");

        let tool = FileWriteTool;
        let ctx = make_ctx(&tmp);
        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "content": "nested content"
        });

        let output = tool.call(input, &ctx).await.unwrap();
        assert!(!output.is_error);

        // Verify parent directories were created and file exists.
        assert!(file_path.exists());
        let written = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(written, "nested content");
    }
}
