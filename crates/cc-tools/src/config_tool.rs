//! ConfigTool -- read and write configuration settings.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::path::PathBuf;

pub struct ConfigTool;

fn config_path(ctx: &ToolContext) -> PathBuf {
    ctx.working_directory.join(".claude").join("settings.json")
}

#[async_trait]
impl Tool for ConfigTool {
    fn name(&self) -> &str {
        "config"
    }

    fn description(&self) -> &str {
        "Get, set, or list configuration values"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["get", "set", "list"], "description": "Action to perform" },
                "key": { "type": "string", "description": "Config key (for get/set)" },
                "value": { "description": "Config value (for set)" }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, input: serde_json::Value, ctx: &ToolContext) -> Result<ToolOutput, CcError> {
        let action = input.get("action").and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("config", "Missing required field: action"))?;

        let path = config_path(ctx);
        let mut config: serde_json::Value = if path.exists() {
            let raw = tokio::fs::read_to_string(&path).await
                .map_err(|e| CcError::tool("config", format!("Read failed: {}", e)))?;
            serde_json::from_str(&raw).unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        match action {
            "get" => {
                let key = input.get("key").and_then(|v| v.as_str())
                    .ok_or_else(|| CcError::tool("config", "Missing key for get"))?;
                let val = config.get(key).cloned().unwrap_or(serde_json::Value::Null);
                Ok(ToolOutput::success(serde_json::to_string_pretty(&val).unwrap_or_default()))
            }
            "set" => {
                let key = input.get("key").and_then(|v| v.as_str())
                    .ok_or_else(|| CcError::tool("config", "Missing key for set"))?;
                let value = input.get("value").cloned()
                    .ok_or_else(|| CcError::tool("config", "Missing value for set"))?;
                config[key] = value;
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await.ok();
                }
                let out = serde_json::to_string_pretty(&config).unwrap_or_default();
                tokio::fs::write(&path, &out).await
                    .map_err(|e| CcError::tool("config", format!("Write failed: {}", e)))?;
                Ok(ToolOutput::success(format!("Set {} = {}", key, config[key])))
            }
            "list" => {
                Ok(ToolOutput::success(serde_json::to_string_pretty(&config).unwrap_or_default()))
            }
            _ => Ok(ToolOutput::error("action must be 'get', 'set', or 'list'")),
        }
    }
}
