//! NotebookEditTool -- edit Jupyter notebook (.ipynb) cells.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::path::Path;

pub struct NotebookEditTool;

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &str {
        "notebook_edit"
    }

    fn description(&self) -> &str {
        "Edit a cell in a Jupyter notebook (.ipynb file)"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "notebook_path": { "type": "string", "description": "Absolute path to the .ipynb file" },
                "new_source": { "type": "string", "description": "New source content for the cell" },
                "cell_number": { "type": "number", "description": "0-based cell index to edit" },
                "cell_type": { "type": "string", "enum": ["code", "markdown"], "description": "Cell type" },
                "edit_mode": { "type": "string", "enum": ["replace", "insert", "delete"], "description": "Edit mode (default: replace)" }
            },
            "required": ["notebook_path", "new_source"]
        })
    }

    async fn call(&self, input: serde_json::Value, ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        let nb_path_str = input.get("notebook_path").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("notebook_edit", "Missing required field: notebook_path"))?;
        let new_source = input.get("new_source").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("notebook_edit", "Missing required field: new_source"))?;
        let cell_number = input.get("cell_number").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let cell_type = input.get("cell_type").and_then(|v| v.as_str()).unwrap_or("code");
        let edit_mode = input.get("edit_mode").and_then(|v| v.as_str()).unwrap_or("replace");

        let nb_path = Path::new(nb_path_str);
        let resolved = if nb_path.is_absolute() { nb_path.to_path_buf() } else { ctx.working_directory.join(nb_path) };

        let raw = tokio::fs::read_to_string(&resolved).await
            .map_err(|e| CcError::tool("notebook_edit", format!("Read failed: {}", e)))?;
        let mut notebook: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| CcError::tool("notebook_edit", format!("Invalid notebook JSON: {}", e)))?;

        let cells = notebook.get_mut("cells").and_then(|v| v.as_array_mut())
            .ok_or_else(|| CcError::tool("notebook_edit", "Notebook has no cells array"))?;

        let source_lines: Vec<serde_json::Value> = new_source
            .lines().map(|l| serde_json::Value::String(format!("{}\n", l))).collect();

        match edit_mode {
            "insert" => {
                let new_cell = serde_json::json!({
                    "cell_type": cell_type, "source": source_lines,
                    "metadata": {}, "outputs": []
                });
                let idx = cell_number.min(cells.len());
                cells.insert(idx, new_cell);
            }
            "delete" => {
                if cell_number >= cells.len() {
                    return Ok(ToolOutput::error(format!("Cell {} out of range ({})", cell_number, cells.len())));
                }
                cells.remove(cell_number);
            }
            _ => {
                if cell_number >= cells.len() {
                    return Ok(ToolOutput::error(format!("Cell {} out of range ({})", cell_number, cells.len())));
                }
                cells[cell_number]["source"] = serde_json::Value::Array(source_lines);
                if let Some(ct) = input.get("cell_type").and_then(|v| v.as_str()) {
                    cells[cell_number]["cell_type"] = serde_json::Value::String(ct.to_string());
                }
            }
        }

        let output_json = serde_json::to_string_pretty(&notebook)
            .map_err(|e| CcError::tool("notebook_edit", format!("Serialize failed: {}", e)))?;
        tokio::fs::write(&resolved, output_json).await
            .map_err(|e| CcError::tool("notebook_edit", format!("Write failed: {}", e)))?;

        Ok(ToolOutput::success(format!("Notebook {} updated (mode: {}, cell: {})", resolved.display(), edit_mode, cell_number)))
    }
}
