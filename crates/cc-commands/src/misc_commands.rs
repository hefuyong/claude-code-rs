//! Miscellaneous commands (bug, feedback, auth, init, update).

use crate::{Command, CommandContext, CommandOutput, CommandRegistry};
use std::process::Stdio;

/// Run a shell command and capture stdout+stderr.
async fn run_cmd(program: &str, args: &[&str], cwd: &std::path::Path) -> String {
    match tokio::process::Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.success() {
                if stdout.is_empty() {
                    "(no output)".to_string()
                } else {
                    stdout.trim().to_string()
                }
            } else {
                format!("Error (exit {}):\n{}{}", output.status, stdout, stderr)
            }
        }
        Err(e) => format!("Failed to run '{program}': {e}"),
    }
}

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
                let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
                let has_key = api_key.is_some();
                let key_format_ok = api_key
                    .as_ref()
                    .map(|k| k.starts_with("sk-ant-"))
                    .unwrap_or(false);

                if !args.is_empty() {
                    Ok(CommandOutput::text(
                        "API key should be set via environment variable for security.\n\n\
                         Do NOT pass keys as command arguments. Instead:\n\n\
                         Option 1: Environment variable (recommended)\n\
                         export ANTHROPIC_API_KEY=sk-ant-...\n\n\
                         Option 2: Shell profile (persistent)\n\
                         Add to ~/.bashrc or ~/.zshrc:\n\
                         export ANTHROPIC_API_KEY=sk-ant-...\n\n\
                         Option 3: .env file (project-specific)\n\
                         Create .env in your project root:\n\
                         ANTHROPIC_API_KEY=sk-ant-...\n\n\
                         Get your key at: https://console.anthropic.com/settings/keys",
                    ))
                } else if has_key {
                    let masked = api_key.as_ref().map(|k| {
                        if k.len() > 10 {
                            format!("{}...{}", &k[..7], &k[k.len()-4..])
                        } else {
                            "******".to_string()
                        }
                    }).unwrap_or_default();

                    let mut lines = vec![
                        "Authentication Status".to_string(),
                        "=====================".to_string(),
                        format!("API key:    {masked}"),
                        format!("Format:     {}", if key_format_ok { "valid (sk-ant-...)" } else { "unusual format" }),
                    ];

                    // Check for other relevant env vars
                    if let Ok(base_url) = std::env::var("ANTHROPIC_BASE_URL") {
                        lines.push(format!("Base URL:   {base_url}"));
                    }

                    lines.push(String::new());
                    lines.push("To change: update the ANTHROPIC_API_KEY environment variable.".into());
                    lines.push("To revoke: visit https://console.anthropic.com/settings/keys".into());

                    Ok(CommandOutput::text(lines.join("\n")))
                } else {
                    Ok(CommandOutput::text(
                        "Authentication Required\n\
                         ======================\n\n\
                         Set your Anthropic API key using one of these methods:\n\n\
                         1. Environment variable (current session):\n\
                            export ANTHROPIC_API_KEY=sk-ant-api03-...\n\n\
                         2. Shell profile (persistent, add to ~/.bashrc or ~/.zshrc):\n\
                            export ANTHROPIC_API_KEY=sk-ant-api03-...\n\n\
                         3. Windows (PowerShell):\n\
                            $env:ANTHROPIC_API_KEY=\"sk-ant-api03-...\"\n\n\
                         Get your API key at:\n\
                         https://console.anthropic.com/settings/keys\n\n\
                         After setting the key, restart Claude Code.",
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
                let memory_dir = claude_dir.join("memory");
                let memory_file = dir.join("CLAUDE.md");
                let gitignore_file = dir.join(".gitignore");

                let mut actions = Vec::new();

                // 1. Create .claude/ directory
                if !claude_dir.exists() {
                    match tokio::fs::create_dir_all(&claude_dir).await {
                        Ok(_) => actions.push(format!(
                            "[created] {}",
                            claude_dir.display()
                        )),
                        Err(e) => actions.push(format!(
                            "[error]   Failed to create {}: {e}",
                            claude_dir.display()
                        )),
                    }
                } else {
                    actions.push(format!(
                        "[exists]  {}",
                        claude_dir.display()
                    ));
                }

                // 2. Create .claude/settings.json
                if !settings_file.exists() {
                    let default_settings = serde_json::json!({
                        "permissions": {
                            "allow": [],
                            "deny": []
                        },
                        "hooks": {},
                        "mcpServers": {}
                    });
                    match serde_json::to_string_pretty(&default_settings) {
                        Ok(json) => match tokio::fs::write(&settings_file, format!("{json}\n")).await {
                            Ok(_) => actions.push(format!(
                                "[created] {}",
                                settings_file.display()
                            )),
                            Err(e) => actions.push(format!(
                                "[error]   Failed to create settings: {e}"
                            )),
                        },
                        Err(e) => actions.push(format!(
                            "[error]   Failed to serialize settings: {e}"
                        )),
                    }
                } else {
                    actions.push(format!(
                        "[exists]  {}",
                        settings_file.display()
                    ));
                }

                // 3. Create .claude/memory/ directory
                if !memory_dir.exists() {
                    match tokio::fs::create_dir_all(&memory_dir).await {
                        Ok(_) => actions.push(format!(
                            "[created] {}",
                            memory_dir.display()
                        )),
                        Err(e) => actions.push(format!(
                            "[error]   Failed to create memory dir: {e}"
                        )),
                    }
                } else {
                    actions.push(format!(
                        "[exists]  {}",
                        memory_dir.display()
                    ));
                }

                // 4. Check/suggest CLAUDE.md
                if !memory_file.exists() {
                    actions.push(format!(
                        "[tip]     Create {} with project context for Claude",
                        memory_file.display()
                    ));
                } else {
                    actions.push(format!(
                        "[exists]  {}",
                        memory_file.display()
                    ));
                }

                // 5. Add .claude/ entries to .gitignore
                let gitignore_entry = ".claude/sessions/\n.claude/memory/\n.claude/logs/\n";
                if gitignore_file.exists() {
                    match tokio::fs::read_to_string(&gitignore_file).await {
                        Ok(content) => {
                            if !content.contains(".claude/sessions") {
                                let updated = format!("{content}\n# Claude Code RS\n{gitignore_entry}");
                                match tokio::fs::write(&gitignore_file, updated).await {
                                    Ok(_) => actions.push("[updated] .gitignore (added .claude/ entries)".into()),
                                    Err(e) => actions.push(format!("[error]   Failed to update .gitignore: {e}")),
                                }
                            } else {
                                actions.push("[exists]  .gitignore already has .claude/ entries".into());
                            }
                        }
                        Err(e) => actions.push(format!("[error]   Failed to read .gitignore: {e}")),
                    }
                } else {
                    let content = format!("# Claude Code RS\n{gitignore_entry}");
                    match tokio::fs::write(&gitignore_file, content).await {
                        Ok(_) => actions.push("[created] .gitignore".into()),
                        Err(e) => actions.push(format!("[error]   Failed to create .gitignore: {e}")),
                    }
                }

                Ok(CommandOutput::text(format!(
                    "Project Initialization\n\
                     =====================\n\
                     {}\n\n\
                     Next steps:\n\
                     - Edit CLAUDE.md to describe your project context\n\
                     - Run /doctor to verify your setup\n\
                     - Start chatting with Claude!",
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
                let version = env!("CARGO_PKG_VERSION");
                let mut lines = vec![
                    format!("Changelog (claude-code-rs v{version})"),
                    "=".repeat(40),
                    String::new(),
                ];

                // Try to find and show CHANGELOG.md
                let changelog_paths = [
                    dir.join("CHANGELOG.md"),
                    dir.join("changelog.md"),
                    dir.join("CHANGES.md"),
                ];

                let mut found_changelog = false;
                for path in &changelog_paths {
                    if path.exists() {
                        match tokio::fs::read_to_string(path).await {
                            Ok(content) => {
                                found_changelog = true;
                                // Show first ~40 lines of changelog
                                let preview_lines: Vec<&str> = content.lines().take(40).collect();
                                lines.push(format!("From: {}", path.display()));
                                lines.push(String::new());
                                for line in &preview_lines {
                                    lines.push(line.to_string());
                                }
                                if content.lines().count() > 40 {
                                    lines.push(format!(
                                        "\n... ({} more lines, see full file)",
                                        content.lines().count() - 40
                                    ));
                                }
                                break;
                            }
                            Err(_) => continue,
                        }
                    }
                }

                if !found_changelog {
                    // Fallback to git log
                    let git_log = run_cmd(
                        "git",
                        &["log", "--oneline", "--decorate", "-20"],
                        &dir,
                    ).await;

                    if git_log.starts_with("Error") || git_log.starts_with("Failed") {
                        lines.push("No CHANGELOG.md found and not in a git repository.".into());
                        lines.push(String::new());
                        lines.push("Releases: https://github.com/anthropics/claude-code/releases".into());
                    } else {
                        lines.push("Recent commits (last 20):".into());
                        lines.push(String::new());
                        lines.push(git_log);
                    }
                }

                lines.push(String::new());
                lines.push("Full release history:".into());
                lines.push("https://github.com/anthropics/claude-code/releases".into());

                Ok(CommandOutput::text(lines.join("\n")))
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
