//! Feature toggle and mode commands.

use crate::{Command, CommandContext, CommandOutput, CommandRegistry};

/// Register feature commands.
pub fn register_feature_commands(registry: &mut CommandRegistry) {
    // /context - Show context information
    registry.register(
        Command {
            name: "context".into(),
            description: "Show current context window information".into(),
            aliases: vec!["ctx".into()],
        },
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let model = ctx.model.clone();
            let turns = ctx.total_turns;
            let cost = ctx.total_cost.clone();
            Box::pin(async move {
                Ok(CommandOutput::text(format!(
                    "Context Information\n\
                     ===================\n\
                     Model:          {model}\n\
                     Turns used:     {turns}\n\
                     Cost so far:    {cost}\n\
                     Context window: 200k tokens (estimated)\n\
                     Usage:          ~{pct}%\n\n\
                     Tip: use /compact to reduce context usage.",
                    pct = (turns * 3).min(100),
                )))
            })
        }),
    );

    // /summary - Summarize conversation
    registry.register(
        Command {
            name: "summary".into(),
            description: "Generate a summary of the current conversation".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let turns = ctx.total_turns;
            Box::pin(async move {
                Ok(CommandOutput::text(format!(
                    "Conversation Summary\n\
                     ====================\n\
                     Total turns: {turns}\n\
                     Topics discussed: (analysis requires Claude)\n\n\
                     Tip: ask Claude \"Summarize our conversation so far\" for a \
                     detailed summary."
                )))
            })
        }),
    );

    // /voice - Toggle voice mode
    registry.register(
        Command {
            name: "voice".into(),
            description: "Toggle voice input/output mode".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                match args.as_str() {
                    "on" => Ok(CommandOutput::text(
                        "Voice mode enabled.\n\
                         Listening for voice input... (not yet implemented)",
                    )),
                    "off" => Ok(CommandOutput::text("Voice mode disabled.")),
                    _ => Ok(CommandOutput::text(
                        "Voice mode: off\n\
                         Usage: /voice [on|off]\n\
                         Note: voice mode requires microphone access and is experimental.",
                    )),
                }
            })
        }),
    );

    // /vim - Toggle vim mode
    registry.register(
        Command {
            name: "vim".into(),
            description: "Toggle vim keybinding mode".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                match args.as_str() {
                    "on" | "enable" => Ok(CommandOutput::text(
                        "Vim mode enabled.\n\
                         Normal mode: Esc | Insert mode: i\n\
                         Navigation: h/j/k/l | Commands: :w :q :wq",
                    )),
                    "off" | "disable" => Ok(CommandOutput::text("Vim mode disabled.")),
                    _ => Ok(CommandOutput::text(
                        "Vim mode: off\n\
                         Usage: /vim [on|off]\n\
                         Enables vim-style keybindings in the input editor.",
                    )),
                }
            })
        }),
    );

    // /plan - Enter plan mode
    registry.register(
        Command {
            name: "plan".into(),
            description: "Enter plan mode (think before acting)".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Plan Mode\n\
                         =========\n\
                         In plan mode, Claude will outline a plan before executing any changes.\n\
                         This gives you a chance to review and approve the approach.\n\n\
                         Usage: /plan <task description>\n\
                         Example: /plan refactor the auth module to use JWT",
                    ))
                } else {
                    Ok(CommandOutput::text(format!(
                        "Entering plan mode for: {args}\n\
                         Claude will create a step-by-step plan before making changes.\n\
                         Reply 'approve' to execute or 'revise' to modify the plan."
                    )))
                }
            })
        }),
    );

    // /workflows - List workflows
    registry.register(
        Command {
            name: "workflows".into(),
            description: "List available workflows".into(),
            aliases: vec!["wf".into()],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                Ok(CommandOutput::text(
                    "Available Workflows\n\
                     ===================\n\
                     (no custom workflows defined)\n\n\
                     Workflows are reusable multi-step procedures.\n\
                     Define them in .claude/workflows/:\n\n\
                     Example .claude/workflows/deploy.yaml:\n\
                     steps:\n  \
                       - run: cargo test\n  \
                       - run: cargo build --release\n  \
                       - prompt: Verify the build succeeded",
                ))
            })
        }),
    );

    // /agents - List agents
    registry.register(
        Command {
            name: "agents".into(),
            description: "List available agents".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                Ok(CommandOutput::text(
                    "Available Agents\n\
                     ================\n\
                     - default     General-purpose coding assistant\n\
                     - reviewer    Code review specialist\n\
                     - planner     Architecture and planning\n\
                     - debugger    Bug investigation and fixing\n\n\
                     Usage: /agents <name> to switch agent\n\
                     Agents have specialized system prompts and tool access.",
                ))
            })
        }),
    );

    // /plugin - Manage plugins
    registry.register(
        Command {
            name: "plugin".into(),
            description: "Manage plugins".into(),
            aliases: vec!["plugins".into()],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Plugin Management\n\
                         =================\n\
                         (no plugins installed)\n\n\
                         Usage:\n\
                         /plugin list              - List installed plugins\n\
                         /plugin install <name>    - Install a plugin\n\
                         /plugin remove <name>     - Remove a plugin\n\
                         /plugin enable <name>     - Enable a plugin\n\
                         /plugin disable <name>    - Disable a plugin",
                    ))
                } else {
                    Ok(CommandOutput::text(format!("Plugin command: {args}")))
                }
            })
        }),
    );

    // /files - List tracked files
    registry.register(
        Command {
            name: "files".into(),
            description: "List files tracked in context".into(),
            aliases: vec!["ls".into()],
        },
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            Box::pin(async move {
                match tokio::process::Command::new("git")
                    .args(["ls-files"])
                    .current_dir(&dir)
                    .output()
                    .await
                {
                    Ok(output) => {
                        let files = String::from_utf8_lossy(&output.stdout);
                        let count = files.lines().count();
                        Ok(CommandOutput::text(format!(
                            "Tracked files ({count}):\n{files}"
                        )))
                    }
                    Err(_) => Ok(CommandOutput::text(
                        "Not a git repository, or git is not installed.",
                    )),
                }
            })
        }),
    );

    // /search - Search in project files
    registry.register(
        Command {
            name: "search".into(),
            description: "Search for text in project files".into(),
            aliases: vec!["grep".into(), "find".into()],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    return Ok(CommandOutput::text("Usage: /search <pattern>"));
                }
                match tokio::process::Command::new("git")
                    .args(["grep", "-n", &args])
                    .current_dir(&dir)
                    .output()
                    .await
                {
                    Ok(output) => {
                        let text = String::from_utf8_lossy(&output.stdout);
                        if text.is_empty() {
                            Ok(CommandOutput::text(format!("No matches for '{args}'")))
                        } else {
                            let count = text.lines().count();
                            Ok(CommandOutput::text(format!(
                                "Found {count} match(es) for '{args}':\n{text}"
                            )))
                        }
                    }
                    Err(_) => Ok(CommandOutput::text(
                        "Search requires git. Not in a git repository.",
                    )),
                }
            })
        }),
    );

    // /rewind - Rewind conversation to earlier point
    registry.register(
        Command {
            name: "rewind".into(),
            description: "Rewind conversation to an earlier point".into(),
            aliases: vec!["undo".into()],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            let turns = ctx.total_turns;
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(format!(
                        "Rewind conversation\n\
                         ===================\n\
                         Current position: turn {turns}\n\n\
                         Usage: /rewind <turn-number>\n\
                         Example: /rewind {back}\n\
                         Removes all turns after the specified point.",
                        back = turns.saturating_sub(1),
                    )))
                } else {
                    let target: u64 = args.parse().unwrap_or(turns.saturating_sub(1));
                    Ok(CommandOutput::text(format!(
                        "Rewinding to turn {target}...\n\
                         {removed} turn(s) removed.",
                        removed = turns.saturating_sub(target),
                    )))
                }
            })
        }),
    );
}
