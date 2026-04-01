//! TodoWriteTool -- write/update a structured todo list.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct TodoWriteTool;

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "todo_write"
    }

    fn description(&self) -> &str {
        "Write or update a structured todo list with items, their IDs, content, and status"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "todos": {
                    "type": "array",
                    "description": "The todo items to write",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string", "description": "Unique todo ID" },
                            "content": { "type": "string", "description": "Todo description" },
                            "status": {
                                "type": "string",
                                "enum": ["pending", "in_progress", "completed"],
                                "description": "Current status"
                            }
                        },
                        "required": ["id", "content", "status"]
                    }
                }
            },
            "required": ["todos"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let todos = input
            .get("todos")
            .and_then(|v| v.as_array())
            .ok_or_else(|| CcError::tool("todo_write", "Missing required field: todos"))?;

        // Persist to a JSON file in the working directory.
        let path = ctx.working_directory.join(".claude").join("todos.json");
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }

        let serialized = serde_json::to_string_pretty(todos)
            .map_err(|e| CcError::tool("todo_write", format!("Serialization error: {}", e)))?;

        tokio::fs::write(&path, &serialized)
            .await
            .map_err(|e| CcError::tool("todo_write", format!("Write error: {}", e)))?;

        Ok(ToolOutput::success(format!(
            "Wrote {} todo item(s) to {}",
            todos.len(),
            path.display()
        )))
    }

    fn is_read_only(&self) -> bool {
        false
    }
}
