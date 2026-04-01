//! TeamDeleteTool -- delete an existing agent team by name.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct TeamDeleteTool;

#[async_trait]
impl Tool for TeamDeleteTool {
    fn name(&self) -> &str {
        "team_delete"
    }

    fn description(&self) -> &str {
        "Delete an agent team by name, removing its configuration"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the team to delete"
                }
            },
            "required": ["name"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("team_delete", "Missing required field: name"))?;

        let team_file = ctx
            .working_directory
            .join(".claude")
            .join("teams")
            .join(format!("{}.json", name));

        if !team_file.exists() {
            return Ok(ToolOutput::error(format!(
                "Team '{}' not found at {}.",
                name,
                team_file.display()
            )));
        }

        tokio::fs::remove_file(&team_file)
            .await
            .map_err(|e| CcError::tool("team_delete", format!("Delete error: {}", e)))?;

        Ok(ToolOutput::success(format!("Deleted team '{}'.", name)))
    }

    fn is_read_only(&self) -> bool {
        false
    }
}
