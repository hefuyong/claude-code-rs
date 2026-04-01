//! ToolSearchTool -- search available tools by keyword.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};

pub struct ToolSearchTool;

/// Static list of (name, description) pairs used for keyword matching.
/// In production this would query the live ToolRegistry.
fn known_tools() -> Vec<(&'static str, &'static str)> {
    vec![
        ("bash", "Execute shell commands"),
        ("file_read", "Read file contents with optional offset"),
        ("file_edit", "Edit files with search and replace"),
        ("file_write", "Write content to files"),
        ("glob", "Find files matching a glob pattern"),
        ("grep", "Search file contents with regex"),
        ("web_fetch", "Fetch and process web page content"),
        ("web_search", "Search the web for information"),
        ("task_stop", "Stop a background task"),
        ("task_output", "Get output from a background task"),
        ("todo_write", "Write a structured todo list"),
        ("sleep", "Pause execution for a duration"),
        ("repl", "Run code in a REPL"),
        ("powershell", "Execute PowerShell commands on Windows"),
        ("lsp", "Language Server Protocol operations"),
        ("mcp_tool", "Call a Model Context Protocol server tool"),
    ]
}

#[async_trait]
impl Tool for ToolSearchTool {
    fn name(&self) -> &str {
        "tool_search"
    }

    fn description(&self) -> &str {
        "Search for available tools by keyword or phrase"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query to match against tool names and descriptions"
                }
            },
            "required": ["query"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let query = input
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("tool_search", "Missing required field: query"))?
            .to_lowercase();

        let matches: Vec<String> = known_tools()
            .into_iter()
            .filter(|(name, desc)| {
                name.to_lowercase().contains(&query)
                    || desc.to_lowercase().contains(&query)
            })
            .map(|(name, desc)| format!("{}: {}", name, desc))
            .collect();

        if matches.is_empty() {
            Ok(ToolOutput::success(format!(
                "No tools matched query '{}'.",
                query
            )))
        } else {
            Ok(ToolOutput::success(format!(
                "{}\n\n({} match(es))",
                matches.join("\n"),
                matches.len()
            )))
        }
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
}
