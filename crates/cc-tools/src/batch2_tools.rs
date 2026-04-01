//! Batch 2 tool registration.
//!
//! This module exports a single function that registers all 15 batch-2 tools
//! into a [`cc_tools_core::ToolRegistry`].
//!
//! ── mod declarations to add to lib.rs ──────────────────────────────────
//!
//! ```ignore
//! pub mod batch2_tools;
//! pub mod brief_tool;
//! pub mod list_mcp_resources;
//! pub mod lsp_tool;
//! pub mod mcp_tool;
//! pub mod powershell;
//! pub mod read_mcp_resource;
//! pub mod repl_tool;
//! pub mod sleep_tool;
//! pub mod synthetic_output;
//! pub mod task_output;
//! pub mod task_stop;
//! pub mod team_create;
//! pub mod team_delete;
//! pub mod todo_write;
//! pub mod tool_search;
//! ```
//!
//! Then call `batch2_tools::register_batch2_tools(&mut registry)` from
//! `register_all_tools()` in lib.rs.

use cc_tools_core::ToolRegistry;

use crate::brief_tool::BriefTool;
use crate::list_mcp_resources::ListMcpResourcesTool;
use crate::lsp_tool::LSPTool;
use crate::mcp_tool::MCPTool;
use crate::powershell::PowerShellTool;
use crate::read_mcp_resource::ReadMcpResourceTool;
use crate::repl_tool::REPLTool;
use crate::sleep_tool::SleepTool;
use crate::synthetic_output::SyntheticOutputTool;
use crate::task_output::TaskOutputTool;
use crate::task_stop::TaskStopTool;
use crate::team_create::TeamCreateTool;
use crate::team_delete::TeamDeleteTool;
use crate::todo_write::TodoWriteTool;
use crate::tool_search::ToolSearchTool;

/// Register all 15 batch-2 tools into the given registry.
pub fn register_batch2_tools(registry: &mut ToolRegistry) {
    registry.register(Box::new(BriefTool));
    registry.register(Box::new(ListMcpResourcesTool));
    registry.register(Box::new(LSPTool));
    registry.register(Box::new(MCPTool));
    registry.register(Box::new(PowerShellTool));
    registry.register(Box::new(ReadMcpResourceTool));
    registry.register(Box::new(REPLTool));
    registry.register(Box::new(SleepTool));
    registry.register(Box::new(SyntheticOutputTool));
    registry.register(Box::new(TaskOutputTool));
    registry.register(Box::new(TaskStopTool));
    registry.register(Box::new(TeamCreateTool));
    registry.register(Box::new(TeamDeleteTool));
    registry.register(Box::new(TodoWriteTool));
    registry.register(Box::new(ToolSearchTool));
}
