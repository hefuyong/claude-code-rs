//! Feature toggle and mode commands.

use crate::{Command, CommandContext, CommandOutput, CommandRegistry};
use std::collections::HashMap;
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
            let dir = ctx.working_dir.clone();
            Box::pin(async move {
                let mut lines = vec![
                    "Context Information".to_string(),
                    "===================".to_string(),
                    String::new(),
                ];

                // Working directory
                lines.push(format!("Working dir:    {}", dir.display()));

                // Git branch
                let branch = run_cmd("git", &["branch", "--show-current"], &dir).await;
                if !branch.starts_with("Error") && !branch.starts_with("Failed") {
                    lines.push(format!("Git branch:     {branch}"));

                    // Git status summary
                    let status = run_cmd("git", &["status", "--porcelain"], &dir).await;
                    if !status.starts_with("Error") && status != "(no output)" {
                        let modified = status.lines().filter(|l| l.starts_with(" M") || l.starts_with("M ")).count();
                        let added = status.lines().filter(|l| l.starts_with("A ") || l.starts_with("??")).count();
                        let deleted = status.lines().filter(|l| l.starts_with(" D") || l.starts_with("D ")).count();
                        lines.push(format!("Git status:     {modified} modified, {added} new, {deleted} deleted"));
                    } else {
                        lines.push("Git status:     clean".into());
                    }
                } else {
                    lines.push("Git:            (not a git repository)".into());
                }

                // File count
                let file_count = run_cmd("git", &["ls-files"], &dir).await;
                if !file_count.starts_with("Error") && !file_count.starts_with("Failed") && file_count != "(no output)" {
                    let count = file_count.lines().count();
                    lines.push(format!("Tracked files:  {count}"));
                }

                lines.push(String::new());

                // Model and session info
                lines.push(format!("Model:          {model}"));
                lines.push(format!("Turns used:     {turns}"));
                lines.push(format!("Cost so far:    {cost}"));

                // Context window estimation
                let context_window: u64 = if model.contains("opus") {
                    200_000
                } else if model.contains("sonnet") {
                    200_000
                } else if model.contains("haiku") {
                    200_000
                } else {
                    200_000
                };
                let est_tokens = turns * 2300; // rough average per turn
                let pct = if context_window > 0 {
                    ((est_tokens as f64 / context_window as f64) * 100.0).min(100.0)
                } else {
                    0.0
                };
                lines.push(format!("Context window: {context_window} tokens"));
                lines.push(format!("Est. usage:     ~{est_tokens} tokens (~{pct:.1}%)"));

                lines.push(String::new());

                // Memory files
                let memory_files = [
                    dir.join("CLAUDE.md"),
                    dir.join(".claude").join("memory.md"),
                    dir.join(".claude").join("CLAUDE.md"),
                ];
                let mut found_memory = false;
                for mf in &memory_files {
                    if mf.exists() {
                        if !found_memory {
                            lines.push("Memory files:".into());
                            found_memory = true;
                        }
                        let size = tokio::fs::metadata(mf).await
                            .map(|m| m.len())
                            .unwrap_or(0);
                        lines.push(format!("  {} ({} bytes)", mf.display(), size));
                    }
                }
                if !found_memory {
                    lines.push("Memory files:   (none found)".into());
                }

                lines.push(String::new());
                lines.push("Tip: use /compact to reduce context usage.".into());

                Ok(CommandOutput::text(lines.join("\n")))
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
            let cost = ctx.total_cost.clone();
            let model = ctx.model.clone();
            Box::pin(async move {
                let est_input = turns * 1500;
                let est_output = turns * 800;
                let est_total = est_input + est_output;

                // Calculate rough pricing
                let input_cost = est_input as f64 * 0.003 / 1000.0;
                let output_cost = est_output as f64 * 0.015 / 1000.0;

                let lines = vec![
                    "Conversation Summary".to_string(),
                    "====================".to_string(),
                    String::new(),
                    format!("Model:            {model}"),
                    format!("Total turns:      {turns}"),
                    format!("  User messages:  ~{}", turns / 2 + turns % 2),
                    format!("  Asst messages:  ~{}", turns / 2),
                    String::new(),
                    "Token Estimates".to_string(),
                    "---------------".to_string(),
                    format!("  Input tokens:   ~{est_input}"),
                    format!("  Output tokens:  ~{est_output}"),
                    format!("  Total tokens:   ~{est_total}"),
                    String::new(),
                    "Cost Breakdown".to_string(),
                    "--------------".to_string(),
                    format!("  Input cost:     ~${input_cost:.4}"),
                    format!("  Output cost:    ~${output_cost:.4}"),
                    format!("  Reported total: {cost}"),
                    String::new(),
                    "Tip: ask Claude \"Summarize our conversation so far\" for a".into(),
                    "content-based summary.".into(),
                ];

                Ok(CommandOutput::text(lines.join("\n")))
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
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                {
                    Ok(output) => {
                        if !output.status.success() {
                            return Ok(CommandOutput::text(
                                "Not a git repository, or git is not installed.",
                            ));
                        }
                        let files = String::from_utf8_lossy(&output.stdout);
                        let total_count = files.lines().count();

                        // Count files by extension
                        let mut ext_counts: HashMap<String, usize> = HashMap::new();
                        for line in files.lines() {
                            let ext = std::path::Path::new(line)
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("(no ext)");
                            *ext_counts.entry(ext.to_string()).or_insert(0) += 1;
                        }

                        // Sort by count descending
                        let mut ext_vec: Vec<(String, usize)> = ext_counts.into_iter().collect();
                        ext_vec.sort_by(|a, b| b.1.cmp(&a.1));

                        let mut lines = vec![
                            format!("Tracked Files ({total_count} total)"),
                            "=".repeat(35),
                            String::new(),
                            "By extension:".to_string(),
                        ];

                        for (ext, count) in &ext_vec {
                            let pct = (*count as f64 / total_count as f64 * 100.0) as u32;
                            lines.push(format!("  .{ext:<12} {count:>5} ({pct}%)"));
                        }

                        // Show top-level directory breakdown
                        let mut dir_counts: HashMap<String, usize> = HashMap::new();
                        for line in files.lines() {
                            let top = line.split('/').next().unwrap_or(".");
                            // Only count if it looks like a directory (file has a slash)
                            if line.contains('/') {
                                *dir_counts.entry(top.to_string()).or_insert(0) += 1;
                            } else {
                                *dir_counts.entry("(root)".to_string()).or_insert(0) += 1;
                            }
                        }
                        let mut dir_vec: Vec<(String, usize)> = dir_counts.into_iter().collect();
                        dir_vec.sort_by(|a, b| b.1.cmp(&a.1));

                        lines.push(String::new());
                        lines.push("By directory:".to_string());
                        for (d, count) in &dir_vec {
                            lines.push(format!("  {d:<16} {count:>5}"));
                        }

                        Ok(CommandOutput::text(lines.join("\n")))
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
                    return Ok(CommandOutput::text(
                        "Usage: /search <pattern>\n\
                         Options:\n\
                         /search <pattern>           - search all files\n\
                         /search -i <pattern>        - case-insensitive\n\
                         /search <pattern> -- *.rs   - search only .rs files"
                    ));
                }

                // Parse arguments: support -i flag and -- file pattern
                let parts: Vec<&str> = args.splitn(2, " -- ").collect();
                let (search_part, file_pattern) = if parts.len() == 2 {
                    (parts[0].trim(), Some(parts[1].trim()))
                } else {
                    (args.as_str(), None)
                };

                let mut git_args = vec!["grep", "-n", "--color=never"];

                let pattern;
                if search_part.starts_with("-i ") {
                    git_args.push("-i");
                    pattern = search_part[3..].trim().to_string();
                } else {
                    pattern = search_part.to_string();
                };
                git_args.push(&pattern);

                if let Some(fp) = file_pattern {
                    git_args.push("--");
                    git_args.push(fp);
                }

                match tokio::process::Command::new("git")
                    .args(&git_args)
                    .current_dir(&dir)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output()
                    .await
                {
                    Ok(output) => {
                        let text = String::from_utf8_lossy(&output.stdout);
                        if text.is_empty() {
                            Ok(CommandOutput::text(format!("No matches for '{pattern}'")))
                        } else {
                            let all_lines: Vec<&str> = text.lines().collect();
                            let count = all_lines.len();

                            // Group by file
                            let mut file_matches: HashMap<&str, Vec<&str>> = HashMap::new();
                            for line in &all_lines {
                                if let Some(colon_pos) = line.find(':') {
                                    let file = &line[..colon_pos];
                                    file_matches.entry(file).or_default().push(line);
                                }
                            }
                            let file_count = file_matches.len();

                            // Show results (cap at 50 lines)
                            let display_lines = if count > 50 {
                                let truncated: String = all_lines[..50].join("\n");
                                format!(
                                    "{truncated}\n\n... and {} more match(es)",
                                    count - 50
                                )
                            } else {
                                text.trim().to_string()
                            };

                            Ok(CommandOutput::text(format!(
                                "Found {count} match(es) in {file_count} file(s) for '{pattern}':\n\n{display_lines}"
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
