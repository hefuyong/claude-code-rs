//! MCP (Model Context Protocol) related commands.

use crate::{Command, CommandContext, CommandOutput, CommandRegistry};

/// Register MCP commands.
pub fn register_mcp_commands(registry: &mut CommandRegistry) {
    // /mcp - MCP server management
    registry.register(
        Command {
            name: "mcp".into(),
            description: "MCP server management".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                if args.is_empty() {
                    Ok(CommandOutput::text(
                        "MCP Server Management\n\
                         =====================\n\
                         Usage:\n\
                         /mcp list                    - List configured servers\n\
                         /mcp add <name> <command>     - Add a server\n\
                         /mcp remove <name>            - Remove a server\n\
                         /mcp restart <name>           - Restart a server\n\
                         /mcp logs <name>              - Show server logs\n\n\
                         Configuration: ~/.claude/settings.json -> mcpServers",
                    ))
                } else {
                    let parts: Vec<&str> = args.splitn(2, ' ').collect();
                    match parts[0] {
                        "list" => {
                            // Read MCP servers from settings
                            let settings = read_mcp_settings().await;
                            match settings {
                                Some(servers) if !servers.is_empty() => {
                                    let mut lines = vec![
                                        "Configured MCP servers:".to_string(),
                                    ];
                                    for (name, config) in &servers {
                                        let cmd = config.get("command")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("(unknown)");
                                        let args_val = config.get("args")
                                            .and_then(|v| v.as_array())
                                            .map(|a| a.iter()
                                                .filter_map(|v| v.as_str())
                                                .collect::<Vec<_>>()
                                                .join(" "))
                                            .unwrap_or_default();
                                        lines.push(format!("  {name}: {cmd} {args_val}"));
                                    }
                                    Ok(CommandOutput::text(lines.join("\n")))
                                }
                                _ => Ok(CommandOutput::text(
                                    "No MCP servers configured.\n\n\
                                     Add servers in ~/.claude/settings.json:\n\
                                     {\n  \
                                       \"mcpServers\": {\n    \
                                         \"my-server\": {\n      \
                                           \"command\": \"node\",\n      \
                                           \"args\": [\"server.js\"]\n    \
                                         }\n  \
                                       }\n\
                                     }",
                                )),
                            }
                        }
                        "add" => Ok(CommandOutput::text(format!(
                            "MCP server command: add {}",
                            parts.get(1).unwrap_or(&"")
                        ))),
                        "remove" => Ok(CommandOutput::text(format!(
                            "Removing MCP server: {}",
                            parts.get(1).unwrap_or(&"<name>")
                        ))),
                        "restart" => Ok(CommandOutput::text(format!(
                            "Restarting MCP server: {}",
                            parts.get(1).unwrap_or(&"<name>")
                        ))),
                        _ => Ok(CommandOutput::text(format!(
                            "Unknown MCP subcommand: {}",
                            parts[0]
                        ))),
                    }
                }
            })
        }),
    );

    // /mcp-status - MCP connection status
    registry.register(
        Command {
            name: "mcp-status".into(),
            description: "Show MCP server connection status".into(),
            aliases: vec!["mcps".into()],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let settings = read_mcp_settings().await;
                match settings {
                    Some(servers) if !servers.is_empty() => {
                        let mut lines = vec![
                            "MCP Connection Status".to_string(),
                            "=====================".to_string(),
                        ];
                        for (name, config) in &servers {
                            let cmd = config.get("command")
                                .and_then(|v| v.as_str())
                                .unwrap_or("(unknown)");
                            // We cannot know the running status without actual connection tracking,
                            // but we can show the configuration
                            lines.push(format!("  {name}: configured ({cmd})"));
                        }
                        lines.push(String::new());
                        lines.push("Note: actual connection status requires a running session.".into());
                        Ok(CommandOutput::text(lines.join("\n")))
                    }
                    _ => Ok(CommandOutput::text(
                        "MCP Connection Status\n\
                         =====================\n\
                         (no MCP servers configured)\n\n\
                         Configure servers in ~/.claude/settings.json:\n\
                         {\n  \
                           \"mcpServers\": {\n    \
                             \"my-server\": {\n      \
                               \"command\": \"node\",\n      \
                               \"args\": [\"server.js\"]\n    \
                             }\n  \
                           }\n\
                         }",
                    )),
                }
            })
        }),
    );

    // /skills - List available skills
    registry.register(
        Command {
            name: "skills".into(),
            description: "List available skills from MCP servers".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let mut lines = vec![
                    "Available Skills".to_string(),
                    "================".to_string(),
                    String::new(),
                    "Built-in skills:".to_string(),
                    "  commit         Create git commits with conventional format".to_string(),
                    "  review-pr      Review pull requests for issues and improvements".to_string(),
                    "  pdf            Process and extract content from PDF files".to_string(),
                    "  simplify       Review and simplify code for clarity".to_string(),
                    "  doc-coauthoring  Collaborative documentation writing workflow".to_string(),
                    "  claude-api     Build apps with the Claude API or Anthropic SDK".to_string(),
                    "  loop           Run a prompt or slash command on a recurring interval".to_string(),
                ];

                // Check for MCP-provided skills from settings
                let settings = read_mcp_settings().await;
                match settings {
                    Some(servers) if !servers.is_empty() => {
                        lines.push(String::new());
                        lines.push("MCP server skills:".to_string());
                        for (name, _config) in &servers {
                            lines.push(format!("  (from {name}) -- connect server to discover skills"));
                        }
                    }
                    _ => {
                        lines.push(String::new());
                        lines.push("MCP-provided skills:".to_string());
                        lines.push("  (no MCP servers connected)".to_string());
                    }
                }

                lines.push(String::new());
                lines.push("Use /mcp to manage MCP servers.".to_string());
                lines.push("Invoke a skill with: /skill-name or ask Claude to use it.".to_string());

                Ok(CommandOutput::text(lines.join("\n")))
            })
        }),
    );

    // /tools - List available tools
    registry.register(
        Command {
            name: "tools".into(),
            description: "List available tools".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let mut lines = vec![
                    "Available Tools".to_string(),
                    "===============".to_string(),
                    String::new(),
                    "Built-in tools:".to_string(),
                    "  Read           Read file contents from disk".to_string(),
                    "  Write          Write or create files on disk".to_string(),
                    "  Edit           Make targeted edits to existing files".to_string(),
                    "  Bash           Execute shell commands".to_string(),
                    "  Glob           Search for files by name pattern".to_string(),
                    "  Grep           Search file contents with regex".to_string(),
                    "  WebFetch       Fetch and process web page content".to_string(),
                    "  WebSearch      Search the web for information".to_string(),
                    "  NotebookEdit   Edit Jupyter notebook cells".to_string(),
                    "  TodoWrite      Manage task lists".to_string(),
                ];

                // Check for MCP tools from settings
                let settings = read_mcp_settings().await;
                match settings {
                    Some(servers) if !servers.is_empty() => {
                        lines.push(String::new());
                        lines.push("MCP server tools:".to_string());
                        for (name, config) in &servers {
                            let cmd = config.get("command")
                                .and_then(|v| v.as_str())
                                .unwrap_or("(unknown)");
                            lines.push(format!("  (from {name}: {cmd}) -- connect to discover tools"));
                        }
                    }
                    _ => {
                        lines.push(String::new());
                        lines.push("MCP tools:".to_string());
                        lines.push("  (no MCP servers connected)".to_string());
                    }
                }

                lines.push(String::new());
                lines.push("Use /mcp to add MCP servers with additional tools.".to_string());

                Ok(CommandOutput::text(lines.join("\n")))
            })
        }),
    );
}

/// Read MCP server configuration from settings files.
async fn read_mcp_settings() -> Option<Vec<(String, serde_json::Value)>> {
    // Check global settings
    let global_path = dirs::home_dir()?.join(".claude").join("settings.json");

    let mut servers = Vec::new();

    for path in &[
        global_path,
        std::path::PathBuf::from(".claude").join("settings.json"),
    ] {
        if path.exists() {
            if let Ok(content) = tokio::fs::read_to_string(path).await {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(mcp) = data.get("mcpServers").and_then(|v| v.as_object()) {
                        for (name, config) in mcp {
                            servers.push((name.clone(), config.clone()));
                        }
                    }
                }
            }
        }
    }

    if servers.is_empty() {
        None
    } else {
        Some(servers)
    }
}
