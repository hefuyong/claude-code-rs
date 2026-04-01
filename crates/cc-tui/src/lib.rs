//! Terminal UI for Claude Code RS.
//!
//! Built on [ratatui](https://ratatui.rs) and [crossterm](https://docs.rs/crossterm),
//! this crate provides the interactive chat interface: a scrollable
//! message area with virtual scrolling, an input line with optional Vim
//! keybindings, a status bar, search, a task panel, a help overlay,
//! permission dialogs, and integration with the agentic query loop.

pub mod app;
pub mod event;
pub mod theme;
pub mod ui;
pub mod widgets;

use app::{App, AppMode};
use cc_error::CcResult;
use crossterm::event::{KeyCode, KeyModifiers, MouseEventKind};
use event::{AppEvent, EventLoop};

/// Configuration needed to start the TUI.
pub struct TuiConfig {
    /// The model to use (e.g. "claude-sonnet-4-20250514").
    pub model: String,
    /// API key for authentication.
    pub api_key: String,
    /// Base URL for the API.
    pub api_base_url: String,
    /// Working directory for tool execution.
    pub working_directory: std::path::PathBuf,
    /// Whether Vim keybindings are enabled.
    pub vim_mode: bool,
    /// The permission mode to use.
    pub permission_mode: cc_permissions::PermissionMode,
    /// Maximum tokens per API turn.
    pub max_tokens: u32,
    /// Maximum agentic turns per query.
    pub max_turns: u32,
    /// Voice mode configuration, if available.
    pub voice_config: Option<cc_voice::config::VoiceConfig>,
    /// Active output style name (e.g. "concise", "verbose").
    pub output_style: Option<String>,
    /// Whether an upstream proxy is configured.
    pub proxy_configured: bool,
    /// Whether LSP integration is enabled.
    pub lsp_enabled: bool,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            api_key: String::new(),
            api_base_url: "https://api.anthropic.com".to_string(),
            working_directory: std::env::current_dir().unwrap_or_else(|_| ".".into()),
            vim_mode: false,
            permission_mode: cc_permissions::PermissionMode::Default,
            max_tokens: 16384,
            max_turns: 10,
            voice_config: None,
            output_style: None,
            proxy_configured: false,
            lsp_enabled: false,
        }
    }
}

/// Entry point: run the interactive TUI until the user quits.
pub async fn run_tui(config: TuiConfig) -> CcResult<()> {
    // Initialize terminal.
    let mut terminal = ratatui::init();
    terminal
        .clear()
        .map_err(|e| cc_error::CcError::Internal(format!("terminal clear failed: {e}")))?;

    let mut app = App::new(config);
    let mut events = EventLoop::new();

    tracing::info!("TUI started");

    loop {
        // Draw the UI.
        terminal
            .draw(|f| ui::render(f, &app))
            .map_err(|e| cc_error::CcError::Internal(format!("draw failed: {e}")))?;

        // Wait for the next event.
        match events.next().await {
            Some(AppEvent::Key(key)) => {
                // Ctrl+C always exits.
                if key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    break;
                }
                app.handle_input(key).await;
            }
            Some(AppEvent::Mouse(mouse)) => {
                match mouse.kind {
                    MouseEventKind::ScrollUp => app.handle_mouse_scroll(true),
                    MouseEventKind::ScrollDown => app.handle_mouse_scroll(false),
                    _ => {}
                }
            }
            Some(AppEvent::QueryEvent(qe)) => {
                app.handle_query_event(qe);
            }
            Some(AppEvent::Resize(_, _)) => {
                // ratatui handles resize automatically on next draw.
            }
            Some(AppEvent::Tick) => {
                // Advance spinner animation.
                app.on_tick();
            }
            None => break,
        }

        if matches!(app.mode, AppMode::Exiting) {
            break;
        }
    }

    // Restore terminal.
    ratatui::restore();
    tracing::info!("TUI exited");
    Ok(())
}
