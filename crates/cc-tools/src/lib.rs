//! Built-in tools for Claude Code RS.
//!
//! Each tool lives in its own module and implements the
//! [`cc_tools_core::Tool`] trait. Call [`register_all_tools`] to
//! populate a [`cc_tools_core::ToolRegistry`] with every built-in tool.

pub mod agent;
pub mod ask_user;
pub mod bash;
pub mod config_tool;
pub mod enter_plan_mode;
pub mod enter_worktree;
pub mod exit_plan_mode;
pub mod exit_worktree;
pub mod file_edit;
pub mod file_read;
pub mod file_write;
pub mod glob;
pub mod grep;
pub mod notebook_edit;
pub mod schedule_cron;
pub mod send_message;
pub mod skill_tool;
pub mod task_create;
pub mod task_get;
pub mod task_list;
pub mod task_update;
pub mod web_fetch;
pub mod web_search;
// Batch 2 tools
pub mod batch2_tools;
pub mod brief_tool;
pub mod list_mcp_resources;
pub mod lsp_tool;
pub mod mcp_tool;
pub mod powershell;
pub mod read_mcp_resource;
pub mod repl_tool;
pub mod sleep_tool;
pub mod synthetic_output;
pub mod task_output;
pub mod task_stop;
pub mod team_create;
pub mod team_delete;
pub mod todo_write;
pub mod tool_search;

use cc_tools_core::ToolRegistry;

/// Register all built-in tools into the given registry.
pub fn register_all_tools(registry: &mut ToolRegistry) {
    // Original tools
    registry.register(Box::new(bash::BashTool));
    registry.register(Box::new(file_read::FileReadTool));
    registry.register(Box::new(file_edit::FileEditTool));
    registry.register(Box::new(file_write::FileWriteTool));
    registry.register(Box::new(glob::GlobTool));
    registry.register(Box::new(grep::GrepTool));
    registry.register(Box::new(web_fetch::WebFetchTool));
    registry.register(Box::new(web_search::WebSearchTool));
    // Agent & messaging tools
    registry.register(Box::new(agent::AgentTool));
    registry.register(Box::new(ask_user::AskUserQuestionTool));
    registry.register(Box::new(send_message::SendMessageTool));
    registry.register(Box::new(skill_tool::SkillTool));
    // Notebook & config tools
    registry.register(Box::new(notebook_edit::NotebookEditTool));
    registry.register(Box::new(config_tool::ConfigTool));
    // Plan & worktree tools
    registry.register(Box::new(enter_plan_mode::EnterPlanModeTool));
    registry.register(Box::new(exit_plan_mode::ExitPlanModeTool));
    registry.register(Box::new(enter_worktree::EnterWorktreeTool));
    registry.register(Box::new(exit_worktree::ExitWorktreeTool));
    // Scheduling tools
    registry.register(Box::new(schedule_cron::ScheduleCronTool));
    // Task management tools
    registry.register(Box::new(task_create::TaskCreateTool));
    registry.register(Box::new(task_list::TaskListTool));
    registry.register(Box::new(task_get::TaskGetTool));
    registry.register(Box::new(task_update::TaskUpdateTool));
    // Batch 2 tools
    batch2_tools::register_batch2_tools(registry);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tools_register_without_panic() {
        let mut registry = ToolRegistry::new();
        register_all_tools(&mut registry);

        let names = registry.list();
        assert_eq!(names.len(), 38);
        // Original tools
        assert!(names.contains(&"bash"));
        assert!(names.contains(&"file_read"));
        assert!(names.contains(&"file_edit"));
        assert!(names.contains(&"file_write"));
        assert!(names.contains(&"glob"));
        assert!(names.contains(&"grep"));
        assert!(names.contains(&"web_fetch"));
        assert!(names.contains(&"web_search"));
        // New tools
        assert!(names.contains(&"agent"));
        assert!(names.contains(&"ask_user"));
        assert!(names.contains(&"notebook_edit"));
        assert!(names.contains(&"enter_plan_mode"));
        assert!(names.contains(&"exit_plan_mode"));
        assert!(names.contains(&"enter_worktree"));
        assert!(names.contains(&"exit_worktree"));
        assert!(names.contains(&"config"));
        assert!(names.contains(&"skill"));
        assert!(names.contains(&"send_message"));
        assert!(names.contains(&"schedule_cron"));
        assert!(names.contains(&"task_create"));
        assert!(names.contains(&"task_list"));
        assert!(names.contains(&"task_get"));
        assert!(names.contains(&"task_update"));
    }

    #[test]
    fn api_tool_definitions_generated() {
        let mut registry = ToolRegistry::new();
        register_all_tools(&mut registry);

        let defs = registry.to_api_tools();
        assert_eq!(defs.len(), 38);
        for def in &defs {
            assert!(!def.name.is_empty());
            assert!(!def.description.is_empty());
            assert!(def.input_schema.is_object());
        }
    }
}
