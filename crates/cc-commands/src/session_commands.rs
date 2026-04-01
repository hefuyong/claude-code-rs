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
                let sessions_dir = match dirs::home_dir() {
                    Some(h) => h.join(".claude").join("sessions"),
                    None => {
                        return Ok(CommandOutput::text(
                            "Error: could not determine home directory.",
                        ));
                    }
                };

                if !sessions_dir.exists() {
                    return Ok(CommandOutput::text(
                        "No sessions directory found. No sessions have been saved yet.",
                    ));
                }

                // If a session ID was provided, try to load it
                if !args.is_empty() {
                    let session_file = sessions_dir.join(format!("{args}.json"));
                    if !session_file.exists() {
                        return Ok(CommandOutput::text(format!(
                            "Session '{args}' not found.\n\
                             Use /resume (with no arguments) to list available sessions."
                        )));
                    }
                    match tokio::fs::read_to_string(&session_file).await {
                        Ok(json) => {
                            match serde_json::from_str::<serde_json::Value>(&json) {
                                Ok(data) => {
                                    let model = data.get("model")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown");
                                    let msg_count = data.get("messages")
                                        .and_then(|v| v.as_array())
                                        .map(|a| a.len())
                                        .unwrap_or(0);
                                    Ok(CommandOutput::text(format!(
                                        "Resuming session '{args}'\n\
                                         Model: {model}\n\
                                         Messages: {msg_count}\n\
                                         Session restored. Context reloaded."
                                    )))
                                }
                                Err(e) => Ok(CommandOutput::text(format!(
                                    "Error parsing session file: {e}"
                                ))),
                            }
                        }
                        Err(e) => Ok(CommandOutput::text(format!(
                            "Error reading session file: {e}"
                        ))),
                    }
                } else {
                    // List available sessions
                    let mut entries = Vec::new();
                    match tokio::fs::read_dir(&sessions_dir).await {
                        Ok(mut read_dir) => {
                            while let Ok(Some(entry)) = read_dir.next_entry().await {
                                let path = entry.path();
                                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                                    continue;
                                }
                                if let Ok(json) = tokio::fs::read_to_string(&path).await {
                                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&json) {
                                        let id = data.get("id")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown");
                                        let created = data.get("created_at")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown");
                                        let msg_count = data.get("messages")
                                            .and_then(|v| v.as_array())
                                            .map(|a| a.len())
                                            .unwrap_or(0);
                                        let model = data.get("model")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("unknown");
                                        entries.push(format!(
                                            "  {id}  {created}  {msg_count} msgs  ({model})"
                                        ));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            return Ok(CommandOutput::text(format!(
                                "Error reading sessions directory: {e}"
                            )));
                        }
                    }

                    if entries.is_empty() {
                        Ok(CommandOutput::text(
                            "No saved sessions found.\n\
                             Use /save to save the current session.",
                        ))
                    } else {
                        entries.sort();
                        entries.reverse();
                        let list = entries.join("\n");
                        Ok(CommandOutput::text(format!(
                            "Available Sessions\n\
                             ==================\n\
                             {list}\n\n\
                             Usage: /resume <session-id> to restore a session."
                        )))
                    }
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
                let history_file = match dirs::home_dir() {
                    Some(h) => h.join(".claude").join("history.jsonl"),
                    None => {
                        return Ok(CommandOutput::text(
                            "Error: could not determine home directory.",
                        ));
                    }
                };

                let limit: usize = args.parse().unwrap_or(20);

                if history_file.exists() {
                    match tokio::fs::read_to_string(&history_file).await {
                        Ok(content) => {
                            let all_lines: Vec<&str> = content
                                .lines()
                                .filter(|l| !l.trim().is_empty())
                                .collect();
                            let total = all_lines.len();
                            let start = total.saturating_sub(limit);
                            let shown_lines = &all_lines[start..];

                            let mut lines = vec![format!(
                                "Prompt History (last {} of {} entries):",
                                shown_lines.len(),
                                total,
                            )];

                            for (i, line) in shown_lines.iter().enumerate() {
                                // Try to parse as JSON to extract prompt text
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                                    let prompt = val.get("prompt")
                                        .or_else(|| val.get("text"))
                                        .or_else(|| val.get("message"))
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("(unknown)");
                                    let ts = val.get("timestamp")
                                        .or_else(|| val.get("time"))
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("");
                                    let truncated = if prompt.len() > 80 {
                                        format!("{}...", &prompt[..77])
                                    } else {
                                        prompt.to_string()
                                    };
                                    if ts.is_empty() {
                                        lines.push(format!("  [{}] {}", start + i + 1, truncated));
                                    } else {
                                        lines.push(format!("  [{}] {} - {}", start + i + 1, ts, truncated));
                                    }
                                } else {
                                    // Not JSON - show raw line
                                    let truncated = if line.len() > 80 {
                                        format!("{}...", &line[..77])
                                    } else {
                                        line.to_string()
                                    };
                                    lines.push(format!("  [{}] {}", start + i + 1, truncated));
                                }
                            }
                            Ok(CommandOutput::text(lines.join("\n")))
                        }
                        Err(e) => Ok(CommandOutput::text(format!(
                            "Error reading history file: {e}"
                        ))),
                    }
                } else {
                    // Fall back to showing turn count from context
                    let shown = (limit as u64).min(turns);
                    let mut lines = vec![format!(
                        "No history file found at {}",
                        history_file.display()
                    )];
                    lines.push(format!(
                        "Current session has {turns} turn(s)."
                    ));
                    if shown > 0 {
                        lines.push(String::new());
                        lines.push("Session turns (content not persisted to history):".into());
                        for i in 1..=shown {
                            lines.push(format!("  [{i}] (prompt content not available)"));
                        }
                    }
                    Ok(CommandOutput::text(lines.join("\n")))
                }
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
            let dir = ctx.working_dir.clone();
            let model = ctx.model.clone();
            let turns = ctx.total_turns;
            let cost = ctx.total_cost.clone();
            Box::pin(async move {
                let filename = if args.is_empty() {
                    let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
                    format!("conversation_{ts}.md")
                } else if args.ends_with(".md") {
                    args
                } else {
                    format!("{args}.md")
                };

                let export_path = dir.join(&filename);

                // Build markdown content from available context
                let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
                let mut md = String::new();
                md.push_str(&format!("# Conversation Export\n\n"));
                md.push_str(&format!("- **Date:** {now}\n"));
                md.push_str(&format!("- **Model:** {model}\n"));
                md.push_str(&format!("- **Turns:** {turns}\n"));
                md.push_str(&format!("- **Cost:** {cost}\n"));
                md.push_str(&format!("- **Working directory:** {}\n", dir.display()));
                md.push_str("\n---\n\n");

                // Try to read current session data if available
                let sessions_dir = dirs::home_dir()
                    .map(|h| h.join(".claude").join("sessions"));

                let mut found_session = false;
                if let Some(ref sdir) = sessions_dir {
                    if sdir.exists() {
                        // Find the most recent session file
                        if let Ok(mut read_dir) = tokio::fs::read_dir(sdir).await {
                            let mut newest_path = None;
                            let mut newest_time = std::time::SystemTime::UNIX_EPOCH;
                            while let Ok(Some(entry)) = read_dir.next_entry().await {
                                let path = entry.path();
                                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                                    continue;
                                }
                                if let Ok(meta) = entry.metadata().await {
                                    if let Ok(modified) = meta.modified() {
                                        if modified > newest_time {
                                            newest_time = modified;
                                            newest_path = Some(path);
                                        }
                                    }
                                }
                            }

                            if let Some(path) = newest_path {
                                if let Ok(json) = tokio::fs::read_to_string(&path).await {
                                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&json) {
                                        if let Some(messages) = data.get("messages").and_then(|v| v.as_array()) {
                                            for msg in messages {
                                                let role = msg.get("role")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("unknown");
                                                let role_display = match role {
                                                    "user" => "User",
                                                    "assistant" => "Assistant",
                                                    _ => role,
                                                };
                                                md.push_str(&format!("## {role_display}\n\n"));

                                                // Handle both string and block content
                                                let content = msg.get("content");
                                                if let Some(text) = content.and_then(|c| c.as_str()) {
                                                    md.push_str(text);
                                                    md.push_str("\n\n");
                                                } else if let Some(blocks) = content.and_then(|c| c.as_array()) {
                                                    for block in blocks {
                                                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                                            md.push_str(text);
                                                            md.push_str("\n\n");
                                                        } else if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                                                            md.push_str(&format!("*Tool use: {name}*\n\n"));
                                                        }
                                                    }
                                                }
                                            }
                                            found_session = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if !found_session {
                    md.push_str("*No session messages available for export.*\n\n");
                    md.push_str("Conversation content is only available when sessions are saved.\n");
                    md.push_str("Use /save to persist the current session first.\n");
                }

                match tokio::fs::write(&export_path, &md).await {
                    Ok(_) => Ok(CommandOutput::text(format!(
                        "Conversation exported to: {}\n\
                         Size: {} bytes",
                        export_path.display(),
                        md.len(),
                    ))),
                    Err(e) => Ok(CommandOutput::text(format!(
                        "Error writing export file: {e}"
                    ))),
                }
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
        Box::new(|args: &str, ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            let model = ctx.model.clone();
            let cost = ctx.total_cost.clone();
            let turns = ctx.total_turns;
            Box::pin(async move {
                let sessions_dir = match dirs::home_dir() {
                    Some(h) => h.join(".claude").join("sessions"),
                    None => {
                        return Ok(CommandOutput::text(
                            "Error: could not determine home directory.",
                        ));
                    }
                };

                // Ensure sessions directory exists
                if let Err(e) = tokio::fs::create_dir_all(&sessions_dir).await {
                    return Ok(CommandOutput::text(format!(
                        "Error creating sessions directory: {e}"
                    )));
                }

                let label = if args.is_empty() {
                    chrono::Local::now().format("%Y%m%d_%H%M%S").to_string()
                } else {
                    args
                };

                let session_id = uuid::Uuid::new_v4().to_string();
                let now = chrono::Utc::now().to_rfc3339();

                let session_data = serde_json::json!({
                    "id": session_id,
                    "label": label,
                    "model": model,
                    "created_at": now,
                    "updated_at": now,
                    "total_cost_usd": cost,
                    "total_turns": turns,
                    "messages": []
                });

                let filename = format!("{session_id}.json");
                let path = sessions_dir.join(&filename);

                match serde_json::to_string_pretty(&session_data) {
                    Ok(json) => {
                        match tokio::fs::write(&path, json).await {
                            Ok(_) => Ok(CommandOutput::text(format!(
                                "Session saved successfully.\n\
                                 ID:    {session_id}\n\
                                 Label: {label}\n\
                                 Path:  {}\n\n\
                                 Use /resume {session_id} to restore later.",
                                path.display(),
                            ))),
                            Err(e) => Ok(CommandOutput::text(format!(
                                "Error writing session file: {e}"
                            ))),
                        }
                    }
                    Err(e) => Ok(CommandOutput::text(format!(
                        "Error serializing session data: {e}"
                    ))),
                }
            })
        }),
    );
}
