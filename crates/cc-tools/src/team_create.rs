//! TeamCreateTool -- create a named agent team with assigned roles.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct TeamCreateTool;

#[async_trait]
impl Tool for TeamCreateTool {
    fn name(&self) -> &str {
        "team_create"
    }

    fn description(&self) -> &str {
        "Create a new agent team with a name and a list of agents with assigned roles"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name for the new team"
                },
                "agents": {
                    "type": "array",
                    "description": "Agents to include in the team",
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string", "description": "Agent name" },
                            "role": { "type": "string", "description": "Agent role within the team" }
                        },
                        "required": ["name", "role"]
                    }
                }
            },
            "required": ["name", "agents"]
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
            .ok_or_else(|| CcError::tool("team_create", "Missing required field: name"))?;

        let agents = input
            .get("agents")
            .and_then(|v| v.as_array())
            .ok_or_else(|| CcError::tool("team_create", "Missing required field: agents"))?;

        if agents.is_empty() {
            return Err(CcError::tool("team_create", "agents array must not be empty"));
        }

        // Persist team definition to disk.
        let teams_dir = ctx.working_directory.join(".claude").join("teams");
        tokio::fs::create_dir_all(&teams_dir).await.ok();

        let team_file = teams_dir.join(format!("{}.json", name));
        let serialized = serde_json::to_string_pretty(&input)
            .map_err(|e| CcError::tool("team_create", format!("Serialization error: {}", e)))?;

        tokio::fs::write(&team_file, &serialized)
            .await
            .map_err(|e| CcError::tool("team_create", format!("Write error: {}", e)))?;

        let agent_names: Vec<&str> = agents
            .iter()
            .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
            .collect();

        Ok(ToolOutput::success(format!(
            "Created team '{}' with {} agent(s): {}",
            name,
            agents.len(),
            agent_names.join(", ")
        )))
    }

    fn is_read_only(&self) -> bool {
        false
    }
}
