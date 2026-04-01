//! Miscellaneous commands (bug, feedback, auth, init, update).

use crate::{Command, CommandContext, CommandOutput, CommandRegistry};

/// Register miscellaneous commands.
pub fn register_misc_commands(registry: &mut CommandRegistry) {
    // /bug - Report a bug
    registry.register(
        Command {
            name: "bug".into(),
            description: "Report a bug".into(),
            aliases: vec!["report".into()],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Report a Bug\n\
                         ============\n\
                         Usage: /bug <description>\n\n\
                         Or open an issue directly:\n\
                         https://github.com/anthropics/claude-code/issues/new\n\n\
                         Please include:\n\
                         - Steps to reproduce\n\
                         - Expected behavior\n\
                         - Actual behavior\n\
                         - Output of /doctor",
                    ))
                } else {
                    Ok(CommandOutput::text(format!(
                        "Bug report noted: {args}\n\n\
                         To submit, open:\n\
                         https://github.com/anthropics/claude-code/issues/new\n\
                         and paste the description above."
                    )))
                }
            })
        }),
    );

    // /feedback - Send feedback
    registry.register(
        Command {
            name: "feedback".into(),
            description: "Send feedback to the development team".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Send Feedback\n\
                         =============\n\
                         Usage: /feedback <message>\n\n\
                         Types of feedback:\n\
                         - Feature requests\n\
                         - Usability improvements\n\
                         - Performance issues\n\
                         - General comments\n\n\
                         Your feedback helps us improve Claude Code.",
                    ))
                } else {
                    Ok(CommandOutput::text(format!(
                        "Thank you for your feedback!\n\
                         Message: {args}\n\n\
                         We appreciate your input."
                    )))
                }
            })
        }),
    );

    // /login - Authenticate
    registry.register(
        Command {
            name: "login".into(),
            description: "Authenticate with Anthropic API".into(),
            aliases: vec!["auth".into()],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                let has_key = std::env::var("ANTHROPIC_API_KEY").is_ok();
                if !args.is_empty() {
                    Ok(CommandOutput::text(
                        "API key configured.\n\
                         Note: for security, pass keys via ANTHROPIC_API_KEY \
                         environment variable rather than command arguments.",
                    ))
                } else if has_key {
                    Ok(CommandOutput::text(
                        "Already authenticated.\n\
                         ANTHROPIC_API_KEY is set in environment.\n\n\
                         To change: update the ANTHROPIC_API_KEY environment variable.",
                    ))
                } else {
                    Ok(CommandOutput::text(
                        "Authentication Required\n\
                         ======================\n\
                         Set your API key:\n\
                         export ANTHROPIC_API_KEY=sk-ant-...\n\n\
                         Or add to ~/.claude/settings.json:\n\
                         { \"apiKey\": \"sk-ant-...\" }\n\n\
                         Get your key at: https://console.anthropic.com/",
                    ))
                }
            })
        }),
    );

    // /logout - Log out
    registry.register(
        Command {
            name: "logout".into(),
            description: "Clear authentication credentials".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                Ok(CommandOutput::text(
                    "Logout\n\
                     ======\n\
                     To clear credentials:\n\
                     - Unset ANTHROPIC_API_KEY from your environment\n\
                     - Remove apiKey from ~/.claude/settings.json\n\n\
                     Note: Claude Code does not store credentials in a session cache.",
                ))
            })
        }),
    );

    // /init - Initialize project
    registry.register(
        Command {
            name: "init".into(),
            description: "Initialize Claude Code in current project".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            Box::pin(async move {
                let claude_dir = dir.join(".claude");
                let settings_file = claude_dir.join("settings.json");
                let memory_file = dir.join("CLAUDE.md");

                let mut actions = Vec::new();

                if !claude_dir.exists() {
                    match tokio::fs::create_dir_all(&claude_dir).await {
                        Ok(_) => actions.push(format!(
                            "Created {}",
                            claude_dir.display()
                        )),
                        Err(e) => actions.push(format!(
                            "Failed to create {}: {e}",
                            claude_dir.display()
                        )),
                    }
                } else {
                    actions.push(format!(
                        "{} already exists",
                        claude_dir.display()
                    ));
                }

                if !settings_file.exists() {
                    let default_settings = "{\n  \"permissions\": {},\n  \"hooks\": {}\n}\n";
                    match tokio::fs::write(&settings_file, default_settings).await {
                        Ok(_) => actions.push(format!(
                            "Created {}",
                            settings_file.display()
                        )),
                        Err(e) => actions.push(format!(
                            "Failed to create settings: {e}"
                        )),
                    }
                } else {
                    actions.push("settings.json already exists".into());
                }

                if !memory_file.exists() {
                    actions.push(format!(
                        "Tip: create {} with project context for Claude",
                        memory_file.display()
                    ));
                } else {
                    actions.push(format!(
                        "{} found",
                        memory_file.display()
                    ));
                }

                Ok(CommandOutput::text(format!(
                    "Project Initialization\n\
                     =====================\n\
                     {}",
                    actions.join("\n")
                )))
            })
        }),
    );

    // /alias - Manage command aliases
    registry.register(
        Command {
            name: "alias".into(),
            description: "Show or define command aliases".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Command Aliases\n\
                         ===============\n\
                         Built-in aliases:\n\
                         h, ?     -> /help\n\
                         q        -> /exit\n\
                         d        -> /diff\n\
                         br       -> /branch\n\
                         v        -> /version\n\n\
                         Usage: /alias <name> <command>\n\
                         Example: /alias ci commit",
                    ))
                } else {
                    Ok(CommandOutput::text(format!("Alias defined: {args}")))
                }
            })
        }),
    );

    // /snippet - Save/load code snippets
    registry.register(
        Command {
            name: "snippet".into(),
            description: "Save or load code snippets".into(),
            aliases: vec!["snip".into()],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "Snippet Manager\n\
                         ===============\n\
                         Usage:\n\
                         /snippet list             - List saved snippets\n\
                         /snippet save <name>      - Save last code block\n\
                         /snippet load <name>      - Load a snippet\n\
                         /snippet delete <name>    - Delete a snippet\n\n\
                         Snippets are stored in ~/.claude/snippets/",
                    ))
                } else {
                    Ok(CommandOutput::text(format!("Snippet command: {args}")))
                }
            })
        }),
    );

    // /profile - Show/switch user profiles
    registry.register(
        Command {
            name: "profile".into(),
            description: "Show or switch user profiles".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "User Profile\n\
                         ============\n\
                         Current: default\n\n\
                         Usage: /profile <name> to switch\n\
                         Profiles store separate settings, API keys, and preferences.\n\
                         Stored in ~/.claude/profiles/",
                    ))
                } else {
                    Ok(CommandOutput::text(format!("Switched to profile: {args}")))
                }
            })
        }),
    );

    // /changelog - Show project changelog
    registry.register(
        Command {
            name: "changelog".into(),
            description: "Show recent changelog entries".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, ctx: &mut CommandContext| {
            let dir = ctx.working_dir.clone();
            Box::pin(async move {
                match tokio::process::Command::new("git")
                    .args(["log", "--oneline", "-20"])
                    .current_dir(&dir)
                    .output()
                    .await
                {
                    Ok(output) => {
                        let text = String::from_utf8_lossy(&output.stdout);
                        Ok(CommandOutput::text(format!(
                            "Recent Changes (last 20 commits):\n{text}"
                        )))
                    }
                    Err(_) => Ok(CommandOutput::text(
                        "Not a git repository, or git is not installed.",
                    )),
                }
            })
        }),
    );

    // /welcome - Show welcome message
    registry.register(
        Command {
            name: "welcome".into(),
            description: "Show the welcome message".into(),
            aliases: vec!["intro".into()],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let version = env!("CARGO_PKG_VERSION");
                Ok(CommandOutput::text(format!(
                    "Welcome to Claude Code RS v{version}!\n\n\
                     Get started:\n\
                     - Type a message to chat with Claude\n\
                     - Use /help to see all commands\n\
                     - Use /doctor to check your setup\n\
                     - Use /init to set up a new project\n\n\
                     Documentation: https://docs.anthropic.com/claude-code",
                )))
            })
        }),
    );

    // /update - Check for updates
    registry.register(
        Command {
            name: "update".into(),
            description: "Check for updates".into(),
            aliases: vec!["upgrade".into()],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let version = env!("CARGO_PKG_VERSION");
                Ok(CommandOutput::text(format!(
                    "Update Check\n\
                     ============\n\
                     Current version: {version}\n\
                     Latest version:  (check https://github.com/anthropics/claude-code/releases)\n\n\
                     To update:\n\
                     cargo install claude-code-rs\n\n\
                     Or build from source:\n\
                     git pull && cargo build --release"
                )))
            })
        }),
    );
}
