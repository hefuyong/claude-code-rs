//! MCP-specific data types.
//!
//! Defines the core domain objects exchanged during MCP communication:
//! server configuration, tools, resources, prompts, and capabilities.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Server Configuration ───────────────────────────────────────────

/// Configuration for connecting to a single MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// A human-readable name for this server connection.
    pub name: String,
    /// How to connect to the server.
    pub transport: TransportConfig,
    /// Additional environment variables to set for the server process.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Transport configuration variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransportConfig {
    /// Communicate over stdin/stdout of a spawned subprocess.
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// Communicate over HTTP Server-Sent Events.
    Sse { url: String },
    /// Communicate over plain HTTP POST requests.
    Http { url: String },
}

// ── Tools ──────────────────────────────────────────────────────────

/// A tool exposed by an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// The tool's unique name within its server.
    pub name: String,
    /// Human-readable description of what the tool does.
    #[serde(default)]
    pub description: Option<String>,
    /// JSON Schema describing the tool's input parameters.
    #[serde(default)]
    pub input_schema: serde_json::Value,
}

// ── Resources ──────────────────────────────────────────────────────

/// A resource exposed by an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    /// The resource's URI.
    pub uri: String,
    /// Human-readable name.
    pub name: String,
    /// Description of the resource.
    #[serde(default)]
    pub description: Option<String>,
    /// MIME type of the resource content.
    #[serde(default)]
    pub mime_type: Option<String>,
}

// ── Prompts ────────────────────────────────────────────────────────

/// A prompt template exposed by an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPrompt {
    /// The prompt's unique name within its server.
    pub name: String,
    /// Human-readable description of the prompt.
    #[serde(default)]
    pub description: Option<String>,
    /// Arguments that the prompt accepts.
    #[serde(default)]
    pub arguments: Vec<McpPromptArgument>,
}

/// A single argument for an MCP prompt template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptArgument {
    /// The argument name.
    pub name: String,
    /// Description of the argument.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether this argument is required.
    #[serde(default)]
    pub required: bool,
}

// ── Capabilities ───────────────────────────────────────────────────

/// Capabilities reported by an MCP server during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Whether the server supports tools.
    #[serde(default)]
    pub tools: bool,
    /// Whether the server supports resources.
    #[serde(default)]
    pub resources: bool,
    /// Whether the server supports prompts.
    #[serde(default)]
    pub prompts: bool,
}

impl ServerCapabilities {
    /// Parse capabilities from the server's initialize response.
    pub fn from_init_response(value: &serde_json::Value) -> Self {
        let caps = value.get("capabilities").unwrap_or(value);
        Self {
            tools: caps.get("tools").is_some(),
            resources: caps.get("resources").is_some(),
            prompts: caps.get("prompts").is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_config_stdio_serde() {
        let cfg = TransportConfig::Stdio {
            command: "node".into(),
            args: vec!["server.js".into()],
        };
        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(json["type"], "stdio");
        assert_eq!(json["command"], "node");
    }

    #[test]
    fn server_capabilities_parsing() {
        let response = serde_json::json!({
            "capabilities": {
                "tools": { "listChanged": true },
                "resources": {}
            }
        });
        let caps = ServerCapabilities::from_init_response(&response);
        assert!(caps.tools);
        assert!(caps.resources);
        assert!(!caps.prompts);
    }

    #[test]
    fn mcp_tool_optional_description() {
        let json = serde_json::json!({
            "name": "search",
            "input_schema": { "type": "object" }
        });
        let tool: McpTool = serde_json::from_value(json).unwrap();
        assert_eq!(tool.name, "search");
        assert!(tool.description.is_none());
    }

    #[test]
    fn mcp_prompt_with_arguments() {
        let json = serde_json::json!({
            "name": "summarize",
            "description": "Summarize text",
            "arguments": [
                { "name": "text", "required": true },
                { "name": "length", "description": "max words" }
            ]
        });
        let prompt: McpPrompt = serde_json::from_value(json).unwrap();
        assert_eq!(prompt.arguments.len(), 2);
        assert!(prompt.arguments[0].required);
        assert!(!prompt.arguments[1].required);
    }
}
