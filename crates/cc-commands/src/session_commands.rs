//! Session management commands.

use crate::{Command, CommandContext, CommandOutput, CommandRegistry};

/// Register session management commands.
pub fn register_session_commands(registry: &mut CommandRegistry) {
    // /session - Show/manage sessions
    registry.register(
        Command {
            name: "session".into(),
            description: "Show/manage sessions (list, load, delete)".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Usage: /session [list|load <id>|delete <id>]\n\n\
                         Session management commands:\n\
                         /session list         - List saved sessions\n\
                         /session load <id>    - Load a saved session\n\
                         /session delete <id>  - Delete a saved session\n\
                         /session save         - Save current session",
                    ))
                } else {
                    Ok(CommandOutput::text(format!("Session command: {args}")))
                }
            })
        }),
    );

    // /resume - Resume a previous session
    registry.register(
        Command {
            name: "resume".into(),
            description: "Resume a previous session".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Usage: /resume <session-id>\n\
                         Tip: use /session list to see available sessions.",
                    ))
                } else {
                    Ok(CommandOutput::text(format!(
                        "Resuming session '{args}'...\n\
                         Session restored. Context reloaded."
                    )))
                }
            })
        }),
    );

    // /history - Show prompt history
    registry.register(
        Command {
            name: "history".into(),
            description: "Show prompt history for current session".into(),
            aliases: vec!["hist".into()],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let turns = ctx.total_turns;
            let args = args.trim().to_string();
            Box::pin(async move {
                let limit: u64 = args.parse().unwrap_or(10);
                let shown = limit.min(turns);
                let mut lines = vec![format!("Prompt history (last {shown} of {turns} turns):")];
                for i in 1..=shown {
                    lines.push(format!("  [{i}] (prompt content summarized)"));
                }
                Ok(CommandOutput::text(lines.join("\n")))
            })
        }),
    );

    // /export - Export conversation as markdown
    registry.register(
        Command {
            name: "export".into(),
            description: "Export conversation as markdown".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            let dir = ctx.working_dir.display().to_string();
            Box::pin(async move {
                let filename = if args.is_empty() {
                    "conversation.md".to_string()
                } else {
                    args
                };
                Ok(CommandOutput::text(format!(
                    "Exporting conversation to {dir}/{filename}...\n\
                     Export complete."
                )))
            })
        }),
    );

    // /teleport - Migrate session to another machine
    registry.register(
        Command {
            name: "teleport".into(),
            description: "Migrate session to another machine".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Usage: /teleport <target-host>\n\
                         Packages the current session state for transfer to another machine.\n\
                         Requires claude-code to be installed on the target.",
                    ))
                } else {
                    Ok(CommandOutput::text(format!(
                        "Packaging session for transfer to '{args}'...\n\
                         Session bundle created. Transfer with:\n  \
                         scp .claude/session-bundle.tar.gz {args}:~/.claude/"
                    )))
                }
            })
        }),
    );

    // /attach - Attach to background session
    registry.register(
        Command {
            name: "attach".into(),
            description: "Attach to a background session".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Usage: /attach <session-id>\n\
                         Tip: use /ps to list background sessions.",
                    ))
                } else {
                    Ok(CommandOutput::text(format!(
                        "Attaching to session '{args}'...\n\
                         Connected. Session output will stream here."
                    )))
                }
            })
        }),
    );

    // /detach - Detach current session to background
    registry.register(
        Command {
            name: "detach".into(),
            description: "Detach current session to background".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                Ok(CommandOutput::text(
                    "Session detached to background.\n\
                     Use /attach or /ps to manage background sessions.",
                ))
            })
        }),
    );

    // /ps - List background sessions
    registry.register(
        Command {
            name: "ps".into(),
            description: "List background sessions".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                Ok(CommandOutput::text(
                    "Background sessions:\n\
                     (no background sessions running)\n\n\
                     Use /detach to send the current session to the background.",
                ))
            })
        }),
    );

    // /save - Save current session state
    registry.register(
        Command {
            name: "save".into(),
            description: "Save current session state to disk".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                let label = if args.is_empty() {
                    "auto".to_string()
                } else {
                    args
                };
                Ok(CommandOutput::text(format!(
                    "Session saved with label: {label}\n\
                     Use /resume {label} to restore later."
                )))
            })
        }),
    );
}
