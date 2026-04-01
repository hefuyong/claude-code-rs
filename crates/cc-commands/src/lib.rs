//! Slash-command system for Claude Code RS.
//!
//! Provides a registry of built-in commands (e.g. `/help`, `/exit`)
//! that can be invoked from the REPL. Commands are organized into
//! category modules:
//!
//! - **session** – session management (resume, history, export, ...)
//! - **code** – code operations (diff, commit, branch, pr, ...)
//! - **config** – configuration (settings, permissions, env, theme, ...)
//! - **feature** – feature toggles (vim, voice, plan, agents, ...)
//! - **diag** – diagnostics (doctor, version, debug, tokens, ...)
//! - **mcp** – MCP servers and tools (mcp, skills, tools, ...)
//! - **misc** – other (bug, feedback, login, init, update, ...)

use cc_error::CcResult;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

// ── Sub-modules ────────────────────────────────────────────────────

mod session_commands;
mod code_commands;
mod config_commands;
mod feature_commands;
mod diag_commands;
mod mcp_commands;
mod misc_commands;

// ── Core types ──────────────────────────────────────────────────────

/// Metadata for a registered command.
#[derive(Debug, Clone)]
pub struct Command {
    /// The primary name (without the leading `/`).
    pub name: String,
    /// A short description shown in `/help`.
    pub description: String,
    /// Alternative names for this command.
    pub aliases: Vec<String>,
}

/// The function signature for command handlers.
pub type CommandHandler = Box<
    dyn Fn(&str, &mut CommandContext) -> Pin<Box<dyn Future<Output = CcResult<CommandOutput>> + Send>>
        + Send
        + Sync,
>;

/// Runtime context available to every command handler.
pub struct CommandContext {
    /// Current working directory.
    pub working_dir: PathBuf,
    /// The model currently in use.
    pub model: String,
    /// Total cost formatted as a string.
    pub total_cost: String,
    /// Total API turns so far.
    pub total_turns: u64,
}

/// The value returned by a command handler.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    /// Text to display to the user.
    pub text: String,
    /// Whether the REPL should exit after this command.
    pub should_exit: bool,
}

impl CommandOutput {
    /// Create a normal (non-exiting) output.
    pub fn text(msg: impl Into<String>) -> Self {
        Self {
            text: msg.into(),
            should_exit: false,
        }
    }

    /// Create an output that causes the REPL to exit.
    pub fn exit(msg: impl Into<String>) -> Self {
        Self {
            text: msg.into(),
            should_exit: true,
        }
    }
}

// ── Registry ────────────────────────────────────────────────────────

/// Maps command names (and aliases) to their metadata and handler.
pub struct CommandRegistry {
    commands: HashMap<String, (Command, CommandHandler)>,
    /// Alias -> primary name mapping.
    aliases: HashMap<String, String>,
}

impl CommandRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            aliases: HashMap::new(),
        }
    }

    /// Register a command and its handler.
    pub fn register(&mut self, cmd: Command, handler: CommandHandler) {
        for alias in &cmd.aliases {
            self.aliases.insert(alias.clone(), cmd.name.clone());
        }
        self.commands.insert(cmd.name.clone(), (cmd, handler));
    }

    /// Resolve a name (which may be an alias) to the primary command name.
    fn resolve(&self, name: &str) -> Option<String> {
        if self.commands.contains_key(name) {
            Some(name.to_string())
        } else {
            self.aliases.get(name).cloned()
        }
    }

    /// Execute a command by name (or alias). Returns `None` if the command
    /// is not found.
    pub async fn execute(
        &self,
        name: &str,
        args: &str,
        ctx: &mut CommandContext,
    ) -> Option<CcResult<CommandOutput>> {
        let primary = self.resolve(name)?;
        let (_cmd, handler) = self.commands.get(&primary)?;
        Some(handler(args, ctx).await)
    }

    /// List all registered commands.
    pub fn list(&self) -> Vec<&Command> {
        let mut cmds: Vec<&Command> = self.commands.values().map(|(c, _)| c).collect();
        cmds.sort_by_key(|c| &c.name);
        cmds
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Built-in core commands ──────────────────────────────────────────

/// Register the core commands that live in lib.rs itself.
fn register_core_commands(registry: &mut CommandRegistry) {
    // /help  – dynamically generates the full command list
    registry.register(
        Command {
            name: "help".into(),
            description: "Show available commands".into(),
            aliases: vec!["h".into(), "?".into()],
        },
        Box::new(|_args, _ctx| {
            Box::pin(async {
                let text = [
                    "Core Commands",
                    "  /help            Show available commands",
                    "  /clear           Clear conversation history",
                    "  /exit            Exit the application",
                    "  /cost            Show token usage and cost",
                    "  /config          Show current configuration",
                    "  /model           Show or change the model",
                    "  /compact         Compact conversation context",
                    "  /status          Show session status",
                    "  /memory          Show loaded memory files",
                    "",
                    "Session Management",
                    "  /session         Show/manage sessions",
                    "  /resume          Resume a previous session",
                    "  /history         Show prompt history",
                    "  /export          Export conversation as markdown",
                    "  /teleport        Migrate session to another machine",
                    "  /attach          Attach to background session",
                    "  /detach          Detach session to background",
                    "  /ps              List background sessions",
                    "  /save            Save current session state",
                    "",
                    "Code Operations",
                    "  /diff            Show current file changes",
                    "  /review          Review code changes",
                    "  /ultrareview     Deep code review",
                    "  /commit          Create git commit",
                    "  /branch          Show/switch git branch",
                    "  /pr              Create pull request",
                    "  /pr_comments     Show PR comments",
                    "  /stash           Git stash operations",
                    "  /blame           Show git blame for a file",
                    "  /autofix-pr      Auto-fix PR issues",
                    "",
                    "Configuration",
                    "  /settings        Open settings",
                    "  /permissions     Show permission rules",
                    "  /hooks           Show configured hooks",
                    "  /env             Show environment info",
                    "  /theme           Change color theme",
                    "  /color           Set color mode",
                    "  /keybindings     Show keybindings",
                    "  /output-style    Set output style",
                    "",
                    "Features",
                    "  /context         Show context information",
                    "  /summary         Summarize conversation",
                    "  /voice           Toggle voice mode",
                    "  /vim             Toggle vim mode",
                    "  /plan            Enter plan mode",
                    "  /workflows       List workflows",
                    "  /agents          List agents",
                    "  /plugin          Manage plugins",
                    "  /files           List tracked files",
                    "  /search          Search in project files",
                    "  /rewind          Rewind conversation",
                    "",
                    "Diagnostics",
                    "  /doctor          System diagnostics",
                    "  /version         Show version info",
                    "  /debug           Toggle debug mode",
                    "  /logs            Show logs",
                    "  /tokens          Show token usage",
                    "  /health          Quick health check",
                    "  /crash-report    Show crash reports",
                    "  /benchmark       Run performance benchmark",
                    "",
                    "MCP",
                    "  /mcp             MCP server management",
                    "  /mcp-status      MCP connection status",
                    "  /skills          List available skills",
                    "  /tools           List available tools",
                    "",
                    "Other",
                    "  /bug             Report a bug",
                    "  /feedback        Send feedback",
                    "  /login           Authenticate",
                    "  /logout          Log out",
                    "  /init            Initialize project",
                    "  /alias           Manage command aliases",
                    "  /snippet         Save/load code snippets",
                    "  /profile         Show/switch user profiles",
                    "  /changelog       Show recent changes",
                    "  /welcome         Show welcome message",
                    "  /update          Check for updates",
                ]
                .join("\n");
                Ok(CommandOutput::text(text))
            })
        }),
    );

    // /clear
    registry.register(
        Command {
            name: "clear".into(),
            description: "Clear conversation history".into(),
            aliases: vec![],
        },
        Box::new(|_args, _ctx| {
            Box::pin(async { Ok(CommandOutput::text("Conversation cleared.")) })
        }),
    );

    // /exit
    registry.register(
        Command {
            name: "exit".into(),
            description: "Exit the application".into(),
            aliases: vec!["quit".into(), "q".into()],
        },
        Box::new(|_args, _ctx| {
            Box::pin(async { Ok(CommandOutput::exit("Goodbye!")) })
        }),
    );

    // /cost
    registry.register(
        Command {
            name: "cost".into(),
            description: "Show token usage and cost".into(),
            aliases: vec![],
        },
        Box::new(|_args, ctx| {
            let cost = ctx.total_cost.clone();
            let turns = ctx.total_turns;
            Box::pin(async move {
                Ok(CommandOutput::text(format!(
                    "Total cost: {cost}\nTotal turns: {turns}"
                )))
            })
        }),
    );

    // /config
    registry.register(
        Command {
            name: "config".into(),
            description: "Show current configuration".into(),
            aliases: vec![],
        },
        Box::new(|_args, ctx| {
            let model = ctx.model.clone();
            let dir = ctx.working_dir.display().to_string();
            Box::pin(async move {
                Ok(CommandOutput::text(format!(
                    "Model: {model}\nWorking directory: {dir}"
                )))
            })
        }),
    );

    // /model
    registry.register(
        Command {
            name: "model".into(),
            description: "Show or change the current model".into(),
            aliases: vec![],
        },
        Box::new(|args, ctx| {
            let args = args.trim().to_string();
            let current = ctx.model.clone();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(format!("Current model: {current}")))
                } else {
                    Ok(CommandOutput::text(format!("Model set to: {args}")))
                }
            })
        }),
    );

    // /compact
    registry.register(
        Command {
            name: "compact".into(),
            description: "Compact conversation context".into(),
            aliases: vec![],
        },
        Box::new(|_args, _ctx| {
            Box::pin(async { Ok(CommandOutput::text("Conversation compacted.")) })
        }),
    );

    // /status
    registry.register(
        Command {
            name: "status".into(),
            description: "Show session status".into(),
            aliases: vec![],
        },
        Box::new(|_args, ctx| {
            let model = ctx.model.clone();
            let cost = ctx.total_cost.clone();
            let turns = ctx.total_turns;
            let dir = ctx.working_dir.display().to_string();
            Box::pin(async move {
                Ok(CommandOutput::text(format!(
                    "Model: {model}\nCost: {cost}\nTurns: {turns}\nCwd: {dir}"
                )))
            })
        }),
    );

    // /memory
    registry.register(
        Command {
            name: "memory".into(),
            description: "Show loaded memory files".into(),
            aliases: vec![],
        },
        Box::new(|_args, _ctx| {
            Box::pin(async { Ok(CommandOutput::text("Memory files: (scan on next prompt)")) })
        }),
    );
}

// ── Public entry point ──────────────────────────────────────────────

/// Register all built-in slash commands (core + every category module).
pub fn register_builtin_commands(registry: &mut CommandRegistry) {
    register_core_commands(registry);
    session_commands::register_session_commands(registry);
    code_commands::register_code_commands(registry);
    config_commands::register_config_commands(registry);
    feature_commands::register_feature_commands(registry);
    diag_commands::register_diag_commands(registry);
    mcp_commands::register_mcp_commands(registry);
    misc_commands::register_misc_commands(registry);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx() -> CommandContext {
        CommandContext {
            working_dir: PathBuf::from("/tmp"),
            model: "claude-sonnet-4-20250514".into(),
            total_cost: "$0.0012".into(),
            total_turns: 5,
        }
    }

    #[tokio::test]
    async fn help_command() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let mut ctx = make_ctx();
        let result = reg.execute("help", "", &mut ctx).await.unwrap().unwrap();
        assert!(result.text.contains("/help"));
        assert!(result.text.contains("/diff"));
        assert!(result.text.contains("/doctor"));
        assert!(!result.should_exit);
    }

    #[tokio::test]
    async fn exit_command() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let mut ctx = make_ctx();
        let result = reg.execute("exit", "", &mut ctx).await.unwrap().unwrap();
        assert!(result.should_exit);
    }

    #[tokio::test]
    async fn alias_resolution() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let mut ctx = make_ctx();
        let result = reg.execute("q", "", &mut ctx).await.unwrap().unwrap();
        assert!(result.should_exit);
    }

    #[tokio::test]
    async fn unknown_command() {
        let reg = CommandRegistry::new();
        let mut ctx = make_ctx();
        let result = reg.execute("nonexistent", "", &mut ctx).await;
        assert!(result.is_none());
    }

    #[test]
    fn list_commands_has_70() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let cmds = reg.list();
        // 9 core + 9 session + 10 code + 8 config + 11 feature
        // + 8 diag + 4 mcp + 11 misc = 70
        assert_eq!(
            cmds.len(),
            70,
            "Expected exactly 70 commands, got {}",
            cmds.len()
        );
    }

    #[tokio::test]
    async fn version_command() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let mut ctx = make_ctx();
        let result = reg.execute("version", "", &mut ctx).await.unwrap().unwrap();
        assert!(result.text.contains("claude-code-rs"));
        assert!(!result.should_exit);
    }

    #[tokio::test]
    async fn tokens_command() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let mut ctx = make_ctx();
        let result = reg.execute("tokens", "", &mut ctx).await.unwrap().unwrap();
        assert!(result.text.contains("Token Usage"));
        assert!(result.text.contains("$0.0012"));
    }

    #[tokio::test]
    async fn vim_command() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let mut ctx = make_ctx();
        let result = reg.execute("vim", "on", &mut ctx).await.unwrap().unwrap();
        assert!(result.text.contains("Vim mode enabled"));
    }

    #[tokio::test]
    async fn env_command() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let mut ctx = make_ctx();
        let result = reg.execute("env", "", &mut ctx).await.unwrap().unwrap();
        assert!(result.text.contains("Environment"));
        assert!(result.text.contains("Working dir"));
    }

    #[tokio::test]
    async fn new_alias_resolution() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let mut ctx = make_ctx();
        // "d" is alias for "diff"
        let result = reg.execute("d", "", &mut ctx).await;
        assert!(result.is_some());
        // "hist" is alias for "history"
        let result = reg.execute("hist", "", &mut ctx).await;
        assert!(result.is_some());
        // "br" is alias for "branch"
        let result = reg.execute("br", "", &mut ctx).await;
        assert!(result.is_some());
        // "diag" is alias for "doctor"
        let result = reg.execute("diag", "", &mut ctx).await;
        assert!(result.is_some());
    }

    #[test]
    fn test_all_commands_have_descriptions() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        for cmd in reg.list() {
            assert!(
                !cmd.description.is_empty(),
                "Command '{}' has an empty description",
                cmd.name
            );
        }
    }

    #[test]
    fn test_no_duplicate_names() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let cmds = reg.list();
        let mut seen = std::collections::HashSet::new();
        for cmd in &cmds {
            assert!(
                seen.insert(&cmd.name),
                "Duplicate command name: '{}'",
                cmd.name
            );
        }
    }

    #[test]
    fn test_no_duplicate_aliases() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let cmds = reg.list();
        let mut seen = std::collections::HashSet::new();
        for cmd in &cmds {
            for alias in &cmd.aliases {
                assert!(
                    seen.insert(alias.clone()),
                    "Duplicate alias: '{}' (from command '{}')",
                    alias,
                    cmd.name
                );
            }
        }
    }

    #[tokio::test]
    async fn test_clear_command_output() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let mut ctx = make_ctx();
        let result = reg.execute("clear", "", &mut ctx).await.unwrap().unwrap();
        assert!(!result.should_exit);
        assert!(
            result.text.contains("clear") || result.text.contains("Clear"),
            "Expected 'clear' or 'Clear' in output, got: {}",
            result.text
        );
    }

    #[tokio::test]
    async fn test_cost_command_output() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let mut ctx = make_ctx();
        let result = reg.execute("cost", "", &mut ctx).await.unwrap().unwrap();
        assert!(!result.should_exit);
        assert!(
            result.text.contains("$"),
            "Expected '$' in cost output, got: {}",
            result.text
        );
        assert!(result.text.contains("Total cost"));
    }

    #[tokio::test]
    async fn test_status_command_output() {
        let mut reg = CommandRegistry::new();
        register_builtin_commands(&mut reg);
        let mut ctx = make_ctx();
        let result = reg.execute("status", "", &mut ctx).await.unwrap().unwrap();
        assert!(!result.should_exit);
        assert!(
            result.text.contains("Model:"),
            "Expected 'Model:' in status output, got: {}",
            result.text
        );
        assert!(result.text.contains("Cost:"));
        assert!(result.text.contains("Turns:"));
    }
}
