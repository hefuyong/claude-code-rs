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
                        "list" => Ok(CommandOutput::text(
                            "Configured MCP servers:\n(no servers configured)",
                        )),
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
                Ok(CommandOutput::text(
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
                ))
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
                Ok(CommandOutput::text(
                    "Available Skills\n\
                     ================\n\
                     Skills are provided by MCP servers and extend Claude's capabilities.\n\n\
                     Built-in skills:\n\
                     - commit      Create git commits\n\
                     - review-pr   Review pull requests\n\
                     - pdf         Process PDF files\n\
                     - simplify    Review and simplify code\n\n\
                     MCP-provided skills:\n\
                     (no MCP servers connected)\n\n\
                     Use /mcp to manage MCP servers.",
                ))
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
                Ok(CommandOutput::text(
                    "Available Tools\n\
                     ===============\n\
                     Built-in tools:\n\
                     - Read          Read file contents\n\
                     - Write         Write/create files\n\
                     - Edit          Edit existing files\n\
                     - Bash          Execute shell commands\n\
                     - Glob          Search for files by pattern\n\
                     - Grep          Search file contents\n\
                     - WebFetch      Fetch web content\n\
                     - WebSearch     Search the web\n\
                     - NotebookEdit  Edit Jupyter notebooks\n\n\
                     MCP tools:\n\
                     (no MCP servers connected)\n\n\
                     Use /mcp to add MCP servers with additional tools.",
                ))
            })
        }),
    );
}
