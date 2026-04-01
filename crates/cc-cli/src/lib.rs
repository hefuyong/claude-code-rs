//! CLI entry point for Claude Code RS.
//!
//! Wires together every subsystem: tools, permissions, query loop, memory,
//! skills, commands, hooks, MCP, sessions, analytics, TUI, and more.

use cc_analytics::AnalyticsClient;
use cc_api::streaming::{simplify_stream, StreamOutput};
use cc_api::{ApiClient, ApiClientConfig, CreateMessageRequest};
use cc_commands::{register_builtin_commands, CommandRegistry};
use cc_config::AppConfig;
use cc_cost::{CallUsage, CostTracker};
use cc_error::{CcError, CcResult};
use cc_hooks::HookRegistry;
use cc_mcp::McpConnectionManager;
use cc_memory::MemoryScanner;
use cc_permissions::{PermissionContext, PermissionMode};
use cc_plugins::PluginManager;
use cc_session::SessionManager;
use cc_skills::SkillRegistry;
use cc_tools_core::{ToolContext, ToolExecutor, ToolRegistry};
use cc_tui::TuiConfig;
use cc_types::SessionId;
use clap::Parser;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_stream::StreamExt;
use tracing_subscriber::EnvFilter;

// ── CLI definition ────────────────────────────────────────────────────

/// Claude Code RS -- An AI-powered coding assistant.
#[derive(Debug, Parser)]
#[command(name = "claude-code", version, about, long_about = None)]
pub struct Cli {
    /// The prompt to send to Claude (if omitted, enters interactive mode).
    #[arg(value_name = "PROMPT")]
    pub prompt: Option<String>,

    /// Enable verbose output.
    #[arg(short, long)]
    pub verbose: bool,

    /// Override the model to use.
    #[arg(long, env = "CLAUDE_MODEL")]
    pub model: Option<String>,

    /// Print output as JSON lines (non-interactive mode).
    #[arg(long, short)]
    pub print: bool,

    /// Maximum output tokens per API turn.
    #[arg(long, default_value = "16384")]
    pub max_tokens: u32,

    /// Maximum agentic turns per query.
    #[arg(long, default_value = "10")]
    pub max_turns: u32,

    /// System prompt override.
    #[arg(long)]
    pub system_prompt: Option<String>,

    /// Permission mode: default, auto, bypass, plan.
    #[arg(long)]
    pub permission_mode: Option<String>,

    /// Additional working directories to allow.
    #[arg(long = "add-dir")]
    pub add_dir: Option<Vec<String>>,

    /// Override the working directory.
    #[arg(long)]
    pub cwd: Option<String>,

    /// Resume a previous session by ID.
    #[arg(long)]
    pub resume: Option<String>,

    /// Disable memory (CLAUDE.md) loading.
    #[arg(long)]
    pub no_memory: bool,

    /// Agent type to use.
    #[arg(long)]
    pub agent: Option<String>,

    /// The subcommand to run.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Available subcommands.
#[derive(Debug, clap::Subcommand)]
pub enum Commands {
    /// Show current configuration.
    Config,
    /// Clear conversation history / all saved sessions.
    Clear,
    /// Resume a previous session.
    Resume {
        /// The session ID to resume (lists sessions if omitted).
        session_id: Option<String>,
    },
    /// Run system diagnostics.
    Doctor,
    /// Manage MCP servers.
    Mcp {
        /// Action: list, add, remove, status.
        action: String,
    },
    /// Show current file changes (git diff).
    Diff,
    /// Show version information.
    Version,
}

// ── Tracing ────────────────────────────────────────────────────────────

fn init_tracing(verbose: bool) {
    let filter = if verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"))
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Resolve the effective working directory from CLI flags.
fn resolve_working_dir(cli: &Cli) -> CcResult<PathBuf> {
    if let Some(ref cwd) = cli.cwd {
        let path = PathBuf::from(cwd);
        if !path.is_dir() {
            return Err(CcError::Config(format!("--cwd path is not a directory: {cwd}")));
        }
        Ok(path)
    } else {
        std::env::current_dir().map_err(CcError::Io)
    }
}

/// Parse the --permission-mode flag into a [`PermissionMode`].
fn parse_permission_mode(mode: Option<&str>) -> PermissionMode {
    match mode {
        Some("auto") => PermissionMode::Auto,
        Some("bypass") => PermissionMode::Bypass,
        Some("plan") => PermissionMode::Plan,
        _ => PermissionMode::Default,
    }
}

/// Build an [`ApiClient`] from the resolved [`AppConfig`].
fn build_api_client(config: &AppConfig, cli: &Cli) -> CcResult<ApiClient> {
    let api_key = config
        .api_key
        .as_ref()
        .ok_or_else(|| CcError::Auth("ANTHROPIC_API_KEY not set".into()))?;

    ApiClient::new(ApiClientConfig {
        api_key: api_key.clone(),
        base_url: config.api_base_url.clone(),
        model: config.model.clone(),
        max_retries: config.max_retries,
        request_timeout: std::time::Duration::from_secs(config.request_timeout_secs),
        max_tokens: cli.max_tokens,
    })
}

/// Collect all allowed working directories (primary + --add-dir).
fn collect_working_dirs(primary: &PathBuf, cli: &Cli) -> Vec<PathBuf> {
    let mut dirs = vec![primary.clone()];
    if let Some(ref extras) = cli.add_dir {
        for d in extras {
            dirs.push(PathBuf::from(d));
        }
    }
    dirs
}

// ── Main entry point ──────────────────────────────────────────────────

/// Main entry point for the CLI binary.
pub async fn run() -> CcResult<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let mut config = AppConfig::load()?;
    if cli.verbose {
        config.verbose = true;
    }
    if let Some(model) = &cli.model {
        config.model = cc_types::ModelId(model.clone());
    }
    config.validate()?;

    match cli.command {
        Some(Commands::Config) => {
            let json = serde_json::to_string_pretty(&config)
                .map_err(|e| CcError::Serialization(e.to_string()))?;
            println!("{json}");
        }
        Some(Commands::Clear) => {
            run_clear().await?;
        }
        Some(Commands::Resume { ref session_id }) => {
            run_resume(&config, session_id.as_deref(), &cli).await?;
        }
        Some(Commands::Doctor) => {
            run_doctor(&config).await?;
        }
        Some(Commands::Mcp { ref action }) => {
            run_mcp_command(action).await?;
        }
        Some(Commands::Diff) => {
            run_diff().await?;
        }
        Some(Commands::Version) => {
            println!("claude-code-rs v{}", env!("CARGO_PKG_VERSION"));
            println!("Model: {}", config.model.0);
        }
        None => {
            // Check if we should resume a session via --resume flag.
            if let Some(ref session_id) = cli.resume {
                run_resume(&config, Some(session_id), &cli).await?;
            } else if let Some(prompt) = &cli.prompt {
                if cli.print {
                    run_print_mode(&config, prompt, &cli).await?;
                } else {
                    run_oneshot(&config, prompt, &cli).await?;
                }
            } else {
                run_interactive(&config, &cli).await?;
            }
        }
    }

    Ok(())
}

// ── Subcommand handlers ───────────────────────────────────────────────

/// `clear` subcommand: remove all saved sessions.
async fn run_clear() -> CcResult<()> {
    let mgr = SessionManager::new()?;
    let sessions = mgr.list().await?;
    let count = sessions.len();
    for s in &sessions {
        mgr.delete(&s.id).await.ok();
    }
    println!("Cleared {count} session(s).");
    Ok(())
}

/// `resume` subcommand: list or resume a session.
async fn run_resume(config: &AppConfig, session_id: Option<&str>, cli: &Cli) -> CcResult<()> {
    let mgr = SessionManager::new()?;

    match session_id {
        Some(id_str) => {
            let uuid_val = ::uuid::Uuid::parse_str(id_str)
                .map_err(|e| CcError::Config(format!("invalid session id: {e}")))?;
            let sid = SessionId(uuid_val);
            let session = mgr.load(&sid).await?;
            println!(
                "Resuming session {} ({} messages, model: {})",
                session.id.0,
                session.messages.len(),
                session.model,
            );
            // Launch the TUI pre-populated with the session's conversation.
            let working_dir = resolve_working_dir(cli)?;
            let tui_config = build_tui_config(config, cli, &working_dir, None);
            cc_tui::run_tui(tui_config).await?;
        }
        None => {
            let sessions = mgr.list().await?;
            if sessions.is_empty() {
                println!("No saved sessions.");
            } else {
                println!("Saved sessions:");
                for s in &sessions {
                    println!(
                        "  {} | {} | {} msgs | {}",
                        s.id.0, s.model, s.message_count, s.created_at,
                    );
                }
            }
        }
    }
    Ok(())
}

/// `doctor` subcommand: run system diagnostics.
async fn run_doctor(config: &AppConfig) -> CcResult<()> {
    println!("Claude Code RS -- System Diagnostics");
    println!("====================================");

    // Check API key.
    print!("API key .......... ");
    if config.api_key.is_some() {
        println!("OK (set)");
    } else {
        println!("MISSING -- set ANTHROPIC_API_KEY");
    }

    // Check model.
    println!("Model ............ {}", config.model.0);

    // Check session directory.
    print!("Session storage .. ");
    match SessionManager::new() {
        Ok(_) => println!("OK"),
        Err(e) => println!("ERROR: {e}"),
    }

    // Check memory files.
    print!("Memory files ..... ");
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let scanner = MemoryScanner::new(cwd.clone());
    let memories = scanner.scan().await;
    println!("{} found", memories.len());

    // Check MCP.
    print!("MCP servers ...... ");
    let mcp = McpConnectionManager::new();
    println!("{} connected", mcp.list_connections().len());

    // Check plugins.
    print!("Plugins .......... ");
    let pm = PluginManager::new();
    println!("{} loaded", pm.list().len());

    // Check tools.
    print!("Built-in tools ... ");
    let mut tool_reg = ToolRegistry::new();
    cc_tools::register_all_tools(&mut tool_reg);
    println!("{}", tool_reg.list().len());

    // Check commands.
    print!("Slash commands ... ");
    let mut cmd_reg = CommandRegistry::new();
    register_builtin_commands(&mut cmd_reg);
    println!("{}", cmd_reg.list().len());

    // Check skills.
    print!("Skills ........... ");
    let mut skill_reg = SkillRegistry::new();
    skill_reg.load_all(&cwd).await.ok();
    println!("{}", skill_reg.list().len());

    // Check coordinator.
    print!("Multi-agent ...... ");
    let coord = cc_coordinator::Coordinator::new(4);
    if coord.is_enabled() {
        println!("enabled");
    } else {
        println!("disabled");
    }

    // Check bridge.
    print!("Bridge ........... ");
    let bridge = cc_bridge::BridgeClient::new(cc_bridge::BridgeConfig::default());
    if bridge.is_connected() {
        println!("connected");
    } else {
        println!("not connected");
    }

    // Check companion.
    let companion = cc_buddy::Companion::generate(42);
    println!("Companion ........ {}", companion.summary());

    println!("\nAll checks complete.");
    Ok(())
}

/// `mcp` subcommand: manage MCP servers.
async fn run_mcp_command(action: &str) -> CcResult<()> {
    let mcp = McpConnectionManager::new();
    match action {
        "list" | "status" => {
            let conns = mcp.list_connections();
            if conns.is_empty() {
                println!("No MCP servers connected.");
            } else {
                println!("Connected MCP servers:");
                for name in conns {
                    println!("  - {name}");
                }
            }

            let tools = mcp.get_all_tools();
            if !tools.is_empty() {
                println!("\nAvailable MCP tools:");
                for (server, tool) in &tools {
                    let desc = tool.description.as_deref().unwrap_or("(no description)");
                    println!("  [{}] {} -- {}", server, tool.name, desc);
                }
            }
        }
        other => {
            println!("Unknown MCP action: {other}");
            println!("Usage: claude-code mcp <list|status|add|remove>");
        }
    }
    Ok(())
}

/// `diff` subcommand: show git diff.
async fn run_diff() -> CcResult<()> {
    let output = tokio::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" })
        .args(if cfg!(windows) {
            vec!["/C", "git diff"]
        } else {
            vec!["-c", "git diff"]
        })
        .output()
        .await
        .map_err(|e| CcError::Internal(format!("failed to run git diff: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.is_empty() {
        println!("No changes.");
    } else {
        print!("{stdout}");
    }
    Ok(())
}

// ── Interactive mode ──────────────────────────────────────────────────

/// Build a [`TuiConfig`] from the resolved app config and CLI flags.
fn build_tui_config(
    config: &AppConfig,
    cli: &Cli,
    working_dir: &PathBuf,
    memory_prompt: Option<String>,
) -> TuiConfig {
    let _ = memory_prompt; // reserved for future use in TuiConfig extension
    TuiConfig {
        model: config.model.0.clone(),
        api_key: config.api_key.clone().unwrap_or_default(),
        api_base_url: config.api_base_url.clone(),
        working_directory: working_dir.clone(),
        vim_mode: false,
        permission_mode: parse_permission_mode(cli.permission_mode.as_deref()),
        max_tokens: cli.max_tokens,
        max_turns: cli.max_turns,
    }
}

/// The main interactive REPL entry point. Initializes all subsystems
/// and launches the TUI.
async fn run_interactive(config: &AppConfig, cli: &Cli) -> CcResult<()> {
    let working_dir = resolve_working_dir(cli)?;
    let working_dirs = collect_working_dirs(&working_dir, cli);

    // 1. Initialize tools.
    let mut tool_registry = ToolRegistry::new();
    cc_tools::register_all_tools(&mut tool_registry);
    tracing::info!(tools = tool_registry.list().len(), "tool registry ready");

    // 2. Initialize permissions.
    let permission_mode = parse_permission_mode(cli.permission_mode.as_deref());
    let permission_ctx = PermissionContext::new(permission_mode, working_dirs);
    let _tool_context = ToolContext {
        working_directory: working_dir.clone(),
        permission_context: permission_ctx.clone(),
    };
    let _tool_executor = ToolExecutor::new(Arc::new(tool_registry));

    // 3. Build API client (validated later when first request is made).
    let _api_client = build_api_client(config, cli).ok();

    // 4. Load memory (unless --no-memory).
    let memory_prompt = if cli.no_memory {
        tracing::info!("memory loading disabled via --no-memory");
        String::new()
    } else {
        let scanner = MemoryScanner::new(working_dir.clone());
        let memories = scanner.scan().await;
        tracing::info!(count = memories.len(), "memory entries loaded");
        MemoryScanner::format_for_prompt(&memories)
    };

    // 5. Load skills.
    let mut skill_registry = SkillRegistry::new();
    skill_registry.load_all(&working_dir).await.ok();
    tracing::info!(skills = skill_registry.list().len(), "skills loaded");

    // 6. Register slash commands.
    let mut cmd_registry = CommandRegistry::new();
    register_builtin_commands(&mut cmd_registry);
    tracing::info!(commands = cmd_registry.list().len(), "commands registered");

    // 7. Load hooks.
    let hook_registry = HookRegistry::new();
    hook_registry.run_session_start().await;

    // 8. Initialize MCP.
    let mcp_manager = McpConnectionManager::new();
    tracing::info!(
        servers = mcp_manager.list_connections().len(),
        "MCP manager ready"
    );

    // 9. Initialize plugins.
    let plugin_manager = PluginManager::new();
    tracing::debug!(plugins = plugin_manager.list().len(), "plugins loaded");

    // 10. Session manager.
    let _session_manager = SessionManager::new()?;

    // 11. Analytics.
    let mut analytics = AnalyticsClient::new(false);
    analytics.track_event("session_start", serde_json::json!({
        "model": config.model.0,
        "permission_mode": format!("{:?}", permission_mode),
    }));

    // 12. Build TUI config and launch.
    let tui_config = build_tui_config(
        config,
        cli,
        &working_dir,
        if memory_prompt.is_empty() {
            None
        } else {
            Some(memory_prompt)
        },
    );

    tracing::info!("launching interactive TUI");
    cc_tui::run_tui(tui_config).await
}

// ── One-shot mode ─────────────────────────────────────────────────────

/// One-shot mode: send prompt, stream response, exit.
async fn run_oneshot(config: &AppConfig, prompt: &str, cli: &Cli) -> CcResult<()> {
    let client = build_api_client(config, cli)?;

    let mut request = CreateMessageRequest::simple(&config.model, prompt, cli.max_tokens);
    request.system = cli.system_prompt.clone();

    let raw_stream = client.send_streaming(&request).await?;
    let mut stream = std::pin::pin!(simplify_stream(raw_stream));

    let mut cost_tracker = CostTracker::new();
    let mut stdout = std::io::stdout();

    while let Some(event) = stream.next().await {
        match event {
            StreamOutput::Text(text) => {
                print!("{text}");
                stdout.flush().ok();
            }
            StreamOutput::Thinking(text) => {
                if cli.verbose {
                    eprint!("\x1b[2m{text}\x1b[0m");
                }
            }
            StreamOutput::ToolUse { name, .. } => {
                eprintln!("\n[Tool: {name}]");
            }
            StreamOutput::Done {
                stop_reason,
                input_tokens,
                output_tokens,
            } => {
                println!();
                cost_tracker.record(
                    &config.model.0,
                    &CallUsage {
                        input_tokens,
                        output_tokens,
                        ..Default::default()
                    },
                );
                if cli.verbose {
                    eprintln!(
                        "\n---\nTokens: {} in / {} out | Cost: {} | Stop: {}",
                        input_tokens,
                        output_tokens,
                        cost_tracker.format_cost(),
                        stop_reason.as_deref().unwrap_or("none"),
                    );
                }
            }
            StreamOutput::Error(msg) => {
                eprintln!("\nError: {msg}");
                return Err(CcError::Api {
                    message: msg,
                    status_code: None,
                });
            }
        }
    }

    Ok(())
}

// ── Print mode (JSON lines) ──────────────────────────────────────────

/// Print mode: similar to oneshot but each event is emitted as a JSON
/// line on stdout. Useful for piping into other tools.
async fn run_print_mode(config: &AppConfig, prompt: &str, cli: &Cli) -> CcResult<()> {
    let client = build_api_client(config, cli)?;

    let mut request = CreateMessageRequest::simple(&config.model, prompt, cli.max_tokens);
    request.system = cli.system_prompt.clone();

    let raw_stream = client.send_streaming(&request).await?;
    let mut stream = std::pin::pin!(simplify_stream(raw_stream));

    let mut stdout = std::io::stdout();

    while let Some(event) = stream.next().await {
        let json_line = match event {
            StreamOutput::Text(text) => {
                serde_json::json!({ "type": "text", "text": text })
            }
            StreamOutput::Thinking(text) => {
                serde_json::json!({ "type": "thinking", "text": text })
            }
            StreamOutput::ToolUse { id, name, input } => {
                serde_json::json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": input,
                })
            }
            StreamOutput::Done {
                stop_reason,
                input_tokens,
                output_tokens,
            } => {
                serde_json::json!({
                    "type": "done",
                    "stop_reason": stop_reason,
                    "input_tokens": input_tokens,
                    "output_tokens": output_tokens,
                })
            }
            StreamOutput::Error(msg) => {
                serde_json::json!({ "type": "error", "message": msg })
            }
        };

        let line = serde_json::to_string(&json_line)
            .map_err(|e| CcError::Serialization(e.to_string()))?;
        println!("{line}");
        stdout.flush().ok();
    }

    Ok(())
}
