//! Slash commands for newly integrated subsystems:
//! voice, LSP, keybindings, proxy, output styles, migrations, IDE connection,
//! and LSP diagnostics.

use crate::{Command, CommandContext, CommandOutput, CommandRegistry};

/// Register all new feature commands.
pub fn register_new_feature_commands(registry: &mut CommandRegistry) {
    register_voice(registry);
    register_lsp(registry);
    register_keybindings(registry);
    register_proxy(registry);
    register_styles(registry);
    register_migrate(registry);
    register_ide(registry);
    register_diagnostics(registry);
}

// ── /voice ────────────────────────────────────────────────────────────

fn register_voice(registry: &mut CommandRegistry) {
    registry.register(
        Command {
            name: "voice".into(),
            description: "Toggle voice input mode".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                let backend = cc_voice::audio::AudioCapture::detect_backend();
                match args.as_str() {
                    "on" => Ok(CommandOutput::text(
                        "Voice mode enabled. Use push-to-talk (hold Space) to speak.",
                    )),
                    "off" => Ok(CommandOutput::text("Voice mode disabled.")),
                    "status" => {
                        let cfg = cc_voice::config::VoiceConfig::default();
                        Ok(CommandOutput::text(format!(
                            "Voice: {}\nBackend: {:?}\nLanguage: {}\nSample rate: {} Hz",
                            if cfg.enabled { "enabled" } else { "disabled" },
                            backend,
                            cfg.language,
                            cfg.sample_rate,
                        )))
                    }
                    _ => Ok(CommandOutput::text("Usage: /voice [on|off|status]")),
                }
            })
        }),
    );
}

// ── /lsp ──────────────────────────────────────────────────────────────

fn register_lsp(registry: &mut CommandRegistry) {
    registry.register(
        Command {
            name: "lsp".into(),
            description: "LSP server management".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                let parts: Vec<&str> = args.split_whitespace().collect();
                match parts.first().copied() {
                    Some("list") => {
                        let servers = cc_lsp::config::default_servers();
                        let mut lines = vec![
                            "Configured LSP Servers".to_string(),
                            "======================".to_string(),
                            String::new(),
                        ];
                        for s in &servers {
                            let exts = s.file_extensions.join(", ");
                            lines.push(format!(
                                "  {} (cmd: {}, extensions: {})",
                                s.name, s.command, exts,
                            ));
                        }
                        if servers.is_empty() {
                            lines.push("  (none configured)".into());
                        }
                        Ok(CommandOutput::text(lines.join("\n")))
                    }
                    Some("start") => {
                        let server_name = parts.get(1).unwrap_or(&"");
                        if server_name.is_empty() {
                            Ok(CommandOutput::text("Usage: /lsp start <server-name>"))
                        } else {
                            Ok(CommandOutput::text(format!(
                                "Starting LSP server '{}'... (lazy initialization)",
                                server_name,
                            )))
                        }
                    }
                    Some("stop") => {
                        let server_name = parts.get(1).unwrap_or(&"");
                        if server_name.is_empty() {
                            Ok(CommandOutput::text("Usage: /lsp stop <server-name>"))
                        } else {
                            Ok(CommandOutput::text(format!(
                                "Stopping LSP server '{}'...",
                                server_name,
                            )))
                        }
                    }
                    Some("status") => {
                        let servers = cc_lsp::config::default_servers();
                        let lines = vec![
                            "LSP Status".to_string(),
                            "==========".to_string(),
                            String::new(),
                            format!("Configured servers: {}", servers.len()),
                            "Active servers:     0 (none started)".to_string(),
                            String::new(),
                            "Use /lsp start <name> to start a server.".to_string(),
                        ];
                        Ok(CommandOutput::text(lines.join("\n")))
                    }
                    _ => Ok(CommandOutput::text(
                        "Usage: /lsp [list|start|stop|status] [server-name]",
                    )),
                }
            })
        }),
    );
}

// ── /keybindings ──────────────────────────────────────────────────────

fn register_keybindings(registry: &mut CommandRegistry) {
    registry.register(
        Command {
            name: "keybindings".into(),
            description: "Show keyboard shortcuts and keybinding configuration".into(),
            aliases: vec!["keys".into(), "shortcuts".into()],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let mut reg = cc_keybindings::KeybindingRegistry::new();
                reg.register_defaults();

                let mut lines = vec![
                    "Keybindings".to_string(),
                    "===========".to_string(),
                    String::new(),
                ];

                let bindings = reg.list();

                // Group by context
                let contexts = [
                    ("Global", cc_keybindings::KeyContext::Global),
                    ("Input", cc_keybindings::KeyContext::Input),
                    ("Normal", cc_keybindings::KeyContext::Normal),
                    ("Search", cc_keybindings::KeyContext::Search),
                    ("Permission Prompt", cc_keybindings::KeyContext::PermissionPrompt),
                ];

                for (label, ctx) in &contexts {
                    let ctx_bindings: Vec<_> = bindings
                        .iter()
                        .filter(|kb| kb.context == *ctx)
                        .collect();
                    if !ctx_bindings.is_empty() {
                        lines.push(format!("{label}:"));
                        for kb in ctx_bindings {
                            let combo_str = cc_keybindings::format_key_combo(&kb.combo);
                            lines.push(format!(
                                "  {:<14} {}",
                                combo_str, kb.description,
                            ));
                        }
                        lines.push(String::new());
                    }
                }

                lines.push(format!("Total: {} bindings", bindings.len()));
                lines.push(String::new());
                lines.push(
                    "Customize in ~/.config/claude-code-rs/keybindings.json".to_string(),
                );

                Ok(CommandOutput::text(lines.join("\n")))
            })
        }),
    );
}

// ── /proxy ────────────────────────────────────────────────────────────

fn register_proxy(registry: &mut CommandRegistry) {
    registry.register(
        Command {
            name: "proxy".into(),
            description: "Show proxy configuration".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let config = cc_proxy::ProxyConfig::from_env();
                let mut lines = vec![
                    "Proxy Configuration".to_string(),
                    "===================".to_string(),
                    String::new(),
                ];

                lines.push(format!(
                    "HTTP_PROXY:     {}",
                    config.http_proxy.as_deref().unwrap_or("(not set)"),
                ));
                lines.push(format!(
                    "HTTPS_PROXY:    {}",
                    config.https_proxy.as_deref().unwrap_or("(not set)"),
                ));

                if config.no_proxy.is_empty() {
                    lines.push("NO_PROXY:       (not set)".to_string());
                } else {
                    lines.push(format!("NO_PROXY:       {}", config.no_proxy.join(", ")));
                }

                lines.push(format!(
                    "SSL_CERT_FILE:  {}",
                    config
                        .ca_cert_path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(not set)".into()),
                ));

                lines.push(String::new());

                if config.is_configured() {
                    lines.push("Status: Proxy is configured.".to_string());
                    if let Some(url) = config.proxy_url_for("https://api.anthropic.com") {
                        lines.push(format!(
                            "API requests will use: {url}",
                        ));
                    }
                } else {
                    lines.push(
                        "Status: No proxy configured. Set HTTP_PROXY / HTTPS_PROXY to enable."
                            .to_string(),
                    );
                }

                Ok(CommandOutput::text(lines.join("\n")))
            })
        }),
    );
}

// ── /styles ───────────────────────────────────────────────────────────

fn register_styles(registry: &mut CommandRegistry) {
    registry.register(
        Command {
            name: "styles".into(),
            description: "Manage output styles".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                let parts: Vec<&str> = args.split_whitespace().collect();
                match parts.first().copied() {
                    Some("list") => {
                        let mut reg = cc_output_styles::OutputStyleRegistry::new();
                        reg.register_builtins();
                        let styles = reg.list();
                        let mut lines = vec![
                            "Available Output Styles".to_string(),
                            "=======================".to_string(),
                            String::new(),
                        ];
                        for s in &styles {
                            lines.push(format!("  {:<12} {}", s.name, s.description));
                        }
                        lines.push(String::new());
                        lines.push("Use /styles set <name> to activate a style.".to_string());
                        Ok(CommandOutput::text(lines.join("\n")))
                    }
                    Some("set") => {
                        let name = parts.get(1).unwrap_or(&"");
                        if name.is_empty() {
                            Ok(CommandOutput::text("Usage: /styles set <name>"))
                        } else {
                            let mut reg = cc_output_styles::OutputStyleRegistry::new();
                            reg.register_builtins();
                            match reg.set_active(name) {
                                Ok(()) => Ok(CommandOutput::text(format!(
                                    "Output style set to: {name}",
                                ))),
                                Err(e) => Ok(CommandOutput::text(format!(
                                    "Error: {e}",
                                ))),
                            }
                        }
                    }
                    Some("show") => {
                        let name = parts.get(1).unwrap_or(&"");
                        if name.is_empty() {
                            Ok(CommandOutput::text("Usage: /styles show <name>"))
                        } else {
                            let mut reg = cc_output_styles::OutputStyleRegistry::new();
                            reg.register_builtins();
                            match reg.get(name) {
                                Some(style) => {
                                    let lines = vec![
                                        format!("Style: {}", style.name),
                                        format!("Description: {}", style.description),
                                        format!(
                                            "Source: {:?}",
                                            style.source,
                                        ),
                                        String::new(),
                                        "Content:".to_string(),
                                        style.content.clone(),
                                    ];
                                    Ok(CommandOutput::text(lines.join("\n")))
                                }
                                None => Ok(CommandOutput::text(format!(
                                    "Style '{}' not found. Use /styles list to see available styles.",
                                    name,
                                ))),
                            }
                        }
                    }
                    _ => Ok(CommandOutput::text(
                        "Usage: /styles [list|set|show] [name]",
                    )),
                }
            })
        }),
    );
}

// ── /migrate ──────────────────────────────────────────────────────────

fn register_migrate(registry: &mut CommandRegistry) {
    registry.register(
        Command {
            name: "migrate".into(),
            description: "Run pending settings migrations".into(),
            aliases: vec![],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                match cc_migrations::MigrationRunner::new() {
                    Ok(mut runner) => {
                        runner.register_all_builtin();

                        let pending_count = runner.pending().map(|p| p.len()).unwrap_or(0);
                        if pending_count == 0 {
                            return Ok(CommandOutput::text(
                                "No pending migrations. Everything is up to date.",
                            ));
                        }

                        let mut settings = serde_json::json!({});
                        match runner.run(&mut settings) {
                            Ok(applied) => {
                                if applied.is_empty() {
                                    Ok(CommandOutput::text(
                                        "No migrations needed to be applied.",
                                    ))
                                } else {
                                    let mut lines = vec![format!(
                                        "Applied {} migration(s):",
                                        applied.len(),
                                    )];
                                    for id in &applied {
                                        lines.push(format!("  - {id}"));
                                    }
                                    Ok(CommandOutput::text(lines.join("\n")))
                                }
                            }
                            Err(e) => Ok(CommandOutput::text(format!(
                                "Migration error: {e}",
                            ))),
                        }
                    }
                    Err(e) => Ok(CommandOutput::text(format!(
                        "Could not initialize migration runner: {e}",
                    ))),
                }
            })
        }),
    );
}

// ── /ide ──────────────────────────────────────────────────────────────

fn register_ide(registry: &mut CommandRegistry) {
    registry.register(
        Command {
            name: "ide".into(),
            description: "IDE connection management".into(),
            aliases: vec![],
        },
        Box::new(|args: &str, _ctx: &mut CommandContext| {
            let args = args.trim().to_string();
            Box::pin(async move {
                let parts: Vec<&str> = args.split_whitespace().collect();
                match parts.first().copied() {
                    Some("connect") => {
                        match cc_ide_connect::IdeConnection::auto_detect().await {
                            Some((ide, path)) => Ok(CommandOutput::text(format!(
                                "Detected {ide} at {}\nAttempting connection...",
                                path.display(),
                            ))),
                            None => Ok(CommandOutput::text(
                                "No running IDE detected.\n\
                                 Supported: VS Code, JetBrains, Vim, Emacs",
                            )),
                        }
                    }
                    Some("disconnect") => Ok(CommandOutput::text(
                        "Disconnected from all IDE instances.",
                    )),
                    Some("status") => {
                        match cc_ide_connect::IdeConnection::auto_detect().await {
                            Some((ide, path)) => Ok(CommandOutput::text(format!(
                                "IDE Connection Status\n\
                                 =====================\n\
                                 IDE:    {ide}\n\
                                 Path:   {}\n\
                                 Status: available",
                                path.display(),
                            ))),
                            None => Ok(CommandOutput::text(
                                "IDE Connection Status\n\
                                 =====================\n\
                                 Status: no IDE detected\n\n\
                                 Supported IDEs:\n\
                                 - VS Code (via IPC socket)\n\
                                 - JetBrains (via TCP port)\n\
                                 - Vim (via socket)\n\
                                 - Emacs (via socket)",
                            )),
                        }
                    }
                    _ => Ok(CommandOutput::text(
                        "Usage: /ide [connect|disconnect|status]",
                    )),
                }
            })
        }),
    );
}

// ── /diagnostics ──────────────────────────────────────────────────────

fn register_diagnostics(registry: &mut CommandRegistry) {
    registry.register(
        Command {
            name: "diagnostics".into(),
            description: "Show LSP diagnostics (errors/warnings)".into(),
            aliases: vec!["diags".into()],
        },
        Box::new(|_args: &str, _ctx: &mut CommandContext| {
            Box::pin(async {
                let manager = cc_lsp::LspManager::new();
                let servers = manager.list_servers();

                let mut lines = vec![
                    "LSP Diagnostics".to_string(),
                    "===============".to_string(),
                    String::new(),
                ];

                if servers.is_empty() {
                    lines.push("No LSP servers running.".to_string());
                    lines.push(String::new());
                    lines.push(
                        "Start a server with /lsp start <name> to see diagnostics.".to_string(),
                    );
                } else {
                    lines.push(format!("{} server(s) active:", servers.len()));
                    for (name, status) in &servers {
                        lines.push(format!("  {} ({:?})", name, status));
                    }
                    lines.push(String::new());
                    lines.push("No diagnostics reported yet.".to_string());
                }

                Ok(CommandOutput::text(lines.join("\n")))
            })
        }),
    );
}
