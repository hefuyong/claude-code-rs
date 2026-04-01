//! BriefTool -- return brief, one-line descriptions for the named tools.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::collections::HashMap;

pub struct BriefTool;

/// Built-in short descriptions.  In a real build these would be gathered from
/// the registry at runtime; here we keep a static table for fast lookup.
fn brief_descriptions() -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    m.insert("bash", "Execute shell commands");
    m.insert("file_read", "Read file contents");
    m.insert("file_edit", "Edit file with search/replace");
    m.insert("file_write", "Write content to a file");
    m.insert("glob", "Find files by glob pattern");
    m.insert("grep", "Search file contents with regex");
    m.insert("web_fetch", "Fetch content from a URL");
    m.insert("web_search", "Search the web");
    m.insert("task_stop", "Stop a background task");
    m.insert("task_output", "Get background task output");
    m.insert("todo_write", "Write a todo list");
    m.insert("sleep", "Pause execution for N ms");
    m.insert("repl", "Run code in a REPL");
    m.insert("powershell", "Execute PowerShell commands");
    m.insert("lsp", "Language Server Protocol ops");
    m.insert("mcp_tool", "Call an MCP server tool");
    m
}

#[async_trait]
impl Tool for BriefTool {
    fn name(&self) -> &str {
        "brief"
    }

    fn description(&self) -> &str {
        "Return brief one-line descriptions of the requested tools"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "tool_names": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of tool names to describe"
                }
            },
            "required": ["tool_names"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let names = input
            .get("tool_names")
            .and_then(|v| v.as_array())
            .ok_or_else(|| CcError::tool("brief", "Missing required field: tool_names"))?;

        let descs = brief_descriptions();
        let mut lines = Vec::new();
        for name_val in names {
            let name = name_val.as_str().unwrap_or("?");
            let desc = descs.get(name).unwrap_or(&"(unknown tool)");
            lines.push(format!("{}: {}", name, desc));
        }

        Ok(ToolOutput::success(lines.join("\n")))
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
}
