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
    async fn test_edit_replace_string() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("edit_me.txt");
        std::fs::write(&file_path, "Hello World\nFoo Bar\n").unwrap();

        let tool = FileEditTool;
        let ctx = make_ctx(&tmp);
        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "Foo Bar",
            "new_string": "Baz Qux"
        });

        let output = tool.call(input, &ctx).await.unwrap();
        assert!(!output.is_error);
        assert!(output.content.contains("Successfully edited"));

        let result = std::fs::read_to_string(&file_path).unwrap();
        assert!(result.contains("Baz Qux"));
        assert!(!result.contains("Foo Bar"));
    }

    #[tokio::test]
    async fn test_edit_string_not_found() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("no_match.txt");
        std::fs::write(&file_path, "some content here\n").unwrap();

        let tool = FileEditTool;
        let ctx = make_ctx(&tmp);
        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "does not exist",
            "new_string": "replacement"
        });

        let output = tool.call(input, &ctx).await.unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("not found"));
    }

    #[tokio::test]
    async fn test_edit_ambiguous_match() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("ambiguous.txt");
        std::fs::write(&file_path, "hello\nhello\nhello\n").unwrap();

        let tool = FileEditTool;
        let ctx = make_ctx(&tmp);
        let input = serde_json::json!({
            "file_path": file_path.to_str().unwrap(),
            "old_string": "hello",
            "new_string": "world"
        });

        let output = tool.call(input, &ctx).await.unwrap();
        assert!(output.is_error);
        assert!(output.content.contains("3 times"));
    }
}
