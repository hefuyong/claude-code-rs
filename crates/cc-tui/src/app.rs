//! Application state and input handling for the TUI.
//!
//! Manages all mutable state: the message list, input buffer with cursor,
//! input history, search state, task panel visibility, help overlay,
//! permission prompts, themes, and the Vim state machine.

use cc_query::QueryEvent;
use cc_state::{AppState, AppStateStore};
use cc_tasks::{TaskInfo, TaskStatus};
use cc_vim::{VimAction, VimMode, VimState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::theme::Theme;
use crate::TuiConfig;

// ── Top-level mode ─────────────────────────────────────────────────

/// The top-level mode of the application.
pub enum AppMode {
    /// The user is composing input.
    Input,
    /// A query is being processed (streaming response).
    Processing,
    /// A permission prompt is being displayed.
    PermissionPrompt(PermPrompt),
    /// The application is shutting down.
    Exiting,
}

// ── Sub-types ──────────────────────────────────────────────────────

/// A permission prompt waiting for the user's response.
pub struct PermPrompt {
    pub tool_name: String,
    pub message: String,
}

/// A single message shown in the scrollable chat area.
pub struct DisplayMessage {
    /// "user", "assistant", "tool", "error", "thinking", or "system".
    pub role: String,
    /// The text content of the message.
    pub content: String,
    /// Human-readable timestamp.
    pub timestamp: String,
    /// If this is a tool-use or tool-result message, the tool name.
    pub tool_name: Option<String>,
}

/// Known slash commands for completion.
const SLASH_COMMANDS: &[&str] = &[
    "/bug",
    "/clear",
    "/compact",
    "/config",
    "/cost",
    "/doctor",
    "/exit",
    "/help",
    "/init",
    "/login",
    "/logout",
    "/memory",
    "/model",
    "/permissions",
    "/quit",
    "/review",
    "/status",
    "/vim",
];

// ── Main application state ─────────────────────────────────────────

/// Main application state for the TUI.
pub struct App {
    // -- Messages --
    /// All messages displayed in the chat area.
    pub messages: Vec<DisplayMessage>,
    /// Accumulated assistant text for the current streaming response.
    pub current_assistant_text: String,

    // -- Input --
    /// The current input text (may contain newlines for multi-line).
    pub input: String,
    /// Cursor position (byte offset) within `input`.
    pub input_cursor: usize,
    /// Past submitted inputs for history browsing.
    pub input_history: Vec<String>,
    /// Index into `input_history` when browsing (None = not browsing).
    pub history_index: Option<usize>,
    /// Saved input text before entering history browsing.
    pub history_stash: String,

    // -- Scrolling --
    /// Scroll offset for the message area (0 = bottom).
    pub scroll_offset: usize,

    // -- Vim --
    /// Vim state machine for the input line.
    pub vim: VimState,
    /// Whether Vim mode is enabled.
    pub vim_enabled: bool,

    // -- Modes --
    /// Current application mode.
    pub mode: AppMode,

    // -- Search --
    /// Whether the search bar is visible.
    pub search_mode: bool,
    /// Current search query text.
    pub search_query: String,
    /// Indices of matching messages: (message_idx, char_offset).
    pub search_matches: Vec<(usize, usize)>,
    /// Current selected match index.
    pub search_current: usize,

    // -- Task panel --
    /// Whether the side task panel is visible (toggled with Tab).
    pub show_task_panel: bool,
    /// Active background tasks.
    pub tasks: Vec<TaskInfo>,

    // -- Help overlay --
    /// Whether the F1 help overlay is visible.
    pub show_help: bool,

    // -- Theme --
    /// Color theme.
    pub theme: Theme,

    // -- Spinner --
    /// Current spinner animation frame index.
    pub spinner_frame: usize,

    // -- Pending permission --
    /// A permission prompt waiting to be shown as a modal.
    pub pending_permission: Option<PermPrompt>,

    // -- Completion --
    /// Tab-completion candidates currently being cycled.
    pub completion_candidates: Vec<String>,
    /// Current index into `completion_candidates`.
    pub completion_index: usize,
    /// Byte offset of the token being completed.
    pub completion_start: usize,

    // -- Global state --
    /// Global observable state store.
    pub state_store: AppStateStore,
    /// Status text shown in the title bar.
    pub status: String,
    /// The model name for display.
    pub model: String,
    /// Turn count (displayed in title bar).
    pub turn_count: u32,
    /// Total token count.
    pub total_tokens: u64,
    /// Total cost string (displayed in title bar).
    pub cost_display: String,
}

impl App {
    /// Create a new application from the given TUI configuration.
    pub fn new(config: TuiConfig) -> Self {
        let state = AppState::new(config.model.clone(), config.working_directory.clone());
        let state_store = AppStateStore::new(state);

        let mut vim = VimState::new();
        if !config.vim_mode {
            vim.mode = VimMode::Insert;
        }

        Self {
            messages: vec![DisplayMessage {
                role: "system".to_string(),
                content: format!(
                    "Welcome to Claude Code RS. Model: {}. Type a message and press Enter.\n\
                     Press F1 for help, Tab for task panel, Ctrl+F to search.",
                    config.model
                ),
                timestamp: now_str(),
                tool_name: None,
            }],
            current_assistant_text: String::new(),

            input: String::new(),
            input_cursor: 0,
            input_history: Vec::new(),
            history_index: None,
            history_stash: String::new(),

            scroll_offset: 0,

            vim,
            vim_enabled: config.vim_mode,

            mode: AppMode::Input,

            search_mode: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_current: 0,

            show_task_panel: false,
            tasks: Vec::new(),

            show_help: false,

            theme: Theme::dark(),
            spinner_frame: 0,
            pending_permission: None,

            completion_candidates: Vec::new(),
            completion_index: 0,
            completion_start: 0,

            state_store,
            status: "Ready".to_string(),
            model: config.model,
            turn_count: 0,
            total_tokens: 0,
            cost_display: "$0.0000".to_string(),
        }
    }

    // ── Tick handling ──────────────────────────────────────────────

    /// Advance spinner frame -- called from the main loop on each Tick.
    pub fn on_tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
    }

    // ── Key event dispatch ────────────────────────────────────────

    /// Handle a crossterm key event.
    pub async fn handle_input(&mut self, key: KeyEvent) {
        // Ctrl+D always exits.
        if key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.mode = AppMode::Exiting;
            return;
        }

        // F1 toggles help overlay.
        if key.code == KeyCode::F(1) {
            self.show_help = !self.show_help;
            return;
        }

        // If help overlay is showing, Esc or F1 or q closes it.
        if self.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => self.show_help = false,
                _ => {}
            }
            return;
        }

        // Permission prompt handling.
        if let AppMode::PermissionPrompt(_) = &self.mode {
            self.handle_permission_key(key);
            return;
        }

        // Search mode handling.
        if self.search_mode {
            self.handle_search_key(key);
            return;
        }

        // Ctrl+F enters search mode.
        if key.code == KeyCode::Char('f') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.enter_search_mode();
            return;
        }

        // If processing, ignore most input.
        if matches!(self.mode, AppMode::Processing) {
            return;
        }

        // Tab toggles task panel (when not typing a slash command).
        if key.code == KeyCode::Tab && self.input.is_empty() {
            self.show_task_panel = !self.show_task_panel;
            return;
        }

        // Tab completion when input is non-empty.
        if key.code == KeyCode::Tab && !self.input.is_empty() {
            self.handle_tab_completion();
            return;
        }

        // Clear completion state on any non-Tab key.
        if key.code != KeyCode::Tab {
            self.completion_candidates.clear();
        }

        if self.vim_enabled {
            self.handle_vim_key(key);
        } else {
            self.handle_plain_key(key);
        }
    }

    // ── Permission prompt ─────────────────────────────────────────

    fn handle_permission_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.mode = AppMode::Input;
                self.status = "Permission granted".to_string();
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.mode = AppMode::Input;
                self.status = "Permission always granted".to_string();
                self.messages.push(DisplayMessage {
                    role: "system".to_string(),
                    content: "Tool permission set to always allow.".to_string(),
                    timestamp: now_str(),
                    tool_name: None,
                });
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.mode = AppMode::Input;
                self.status = "Permission denied".to_string();
                self.messages.push(DisplayMessage {
                    role: "error".to_string(),
                    content: "Permission denied by user.".to_string(),
                    timestamp: now_str(),
                    tool_name: None,
                });
            }
            _ => {}
        }
    }

    // ── Search mode ───────────────────────────────────────────────

    fn enter_search_mode(&mut self) {
        self.search_mode = true;
        self.search_query.clear();
        self.search_matches.clear();
        self.search_current = 0;
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search_mode = false;
            }
            KeyCode::Enter => {
                // Navigate to next match.
                if !self.search_matches.is_empty() {
                    self.search_current = (self.search_current + 1) % self.search_matches.len();
                    self.scroll_to_search_match();
                }
            }
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Next match.
                if !self.search_matches.is_empty() {
                    self.search_current = (self.search_current + 1) % self.search_matches.len();
                    self.scroll_to_search_match();
                }
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Previous match.
                if !self.search_matches.is_empty() {
                    self.search_current = if self.search_current == 0 {
                        self.search_matches.len() - 1
                    } else {
                        self.search_current - 1
                    };
                    self.scroll_to_search_match();
                }
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.update_search_matches();
            }
            KeyCode::Char(ch) => {
                self.search_query.push(ch);
                self.update_search_matches();
            }
            _ => {}
        }
    }

    fn update_search_matches(&mut self) {
        self.search_matches.clear();
        self.search_current = 0;
        if self.search_query.is_empty() {
            return;
        }
        let query_lower = self.search_query.to_lowercase();
        for (msg_idx, msg) in self.messages.iter().enumerate() {
            let content_lower = msg.content.to_lowercase();
            for (char_offset, _) in content_lower.match_indices(&query_lower) {
                self.search_matches.push((msg_idx, char_offset));
            }
        }
    }

    fn scroll_to_search_match(&mut self) {
        if let Some(&(msg_idx, _)) = self.search_matches.get(self.search_current) {
            // Rough heuristic: scroll so the matching message is visible.
            let total = self.messages.len();
            let distance_from_end = total.saturating_sub(msg_idx + 1);
            self.scroll_offset = distance_from_end.saturating_sub(2);
        }
    }

    // ── Tab completion ────────────────────────────────────────────

    fn handle_tab_completion(&mut self) {
        // If we already have candidates, cycle through them.
        if !self.completion_candidates.is_empty() {
            self.completion_index =
                (self.completion_index + 1) % self.completion_candidates.len();
            let candidate = self.completion_candidates[self.completion_index].clone();
            self.input.truncate(self.completion_start);
            self.input.push_str(&candidate);
            self.input_cursor = self.input.len();
            return;
        }

        // Determine what to complete.
        if self.input.starts_with('/') {
            // Slash command completion.
            let prefix = &self.input;
            let matches: Vec<String> = SLASH_COMMANDS
                .iter()
                .filter(|cmd| cmd.starts_with(prefix.as_str()))
                .map(|s| s.to_string())
                .collect();
            if !matches.is_empty() {
                self.completion_start = 0;
                self.completion_candidates = matches;
                self.completion_index = 0;
                let candidate = self.completion_candidates[0].clone();
                self.input = candidate;
                self.input_cursor = self.input.len();
            }
        }
        // File path completion would go here in a full implementation.
    }

    // ── Plain key handling ────────────────────────────────────────

    fn handle_plain_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+Enter inserts a newline.
                    self.input.insert(self.input_cursor, '\n');
                    self.input_cursor += 1;
                } else {
                    self.submit_message();
                }
            }
            KeyCode::Backspace => {
                if self.input_cursor > 0 {
                    let prev = prev_char_boundary(&self.input, self.input_cursor);
                    self.input.drain(prev..self.input_cursor);
                    self.input_cursor = prev;
                }
            }
            KeyCode::Delete => {
                if self.input_cursor < self.input.len() {
                    let next = next_char_boundary(&self.input, self.input_cursor);
                    self.input.drain(self.input_cursor..next);
                }
            }
            KeyCode::Left => {
                self.input_cursor = prev_char_boundary(&self.input, self.input_cursor);
            }
            KeyCode::Right => {
                if self.input_cursor < self.input.len() {
                    self.input_cursor = next_char_boundary(&self.input, self.input_cursor);
                }
            }
            KeyCode::Home => {
                self.input_cursor = 0;
            }
            KeyCode::End => {
                self.input_cursor = self.input.len();
            }
            KeyCode::Up => {
                self.history_prev();
            }
            KeyCode::Down => {
                self.history_next();
            }
            KeyCode::Char(ch) => {
                self.input.insert(self.input_cursor, ch);
                self.input_cursor += ch.len_utf8();
            }
            KeyCode::PageUp => self.scroll_up(),
            KeyCode::PageDown => self.scroll_down(),
            _ => {}
        }
    }

    // ── Vim key handling ──────────────────────────────────────────

    fn handle_vim_key(&mut self, key: KeyEvent) {
        let ch = match key.code {
            KeyCode::Char(c) => c,
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.input.insert(self.input_cursor, '\n');
                    self.input_cursor += 1;
                    return;
                }
                '\n'
            }
            KeyCode::Backspace => '\x7f',
            KeyCode::Esc => '\x1b',
            KeyCode::PageUp => {
                self.scroll_up();
                return;
            }
            KeyCode::PageDown => {
                self.scroll_down();
                return;
            }
            KeyCode::Left => {
                self.input_cursor = prev_char_boundary(&self.input, self.input_cursor);
                return;
            }
            KeyCode::Right => {
                if self.input_cursor < self.input.len() {
                    self.input_cursor = next_char_boundary(&self.input, self.input_cursor);
                }
                return;
            }
            KeyCode::Up => {
                self.history_prev();
                return;
            }
            KeyCode::Down => {
                self.history_next();
                return;
            }
            _ => return,
        };

        let action = self.vim.handle_key(ch, &self.input, self.input_cursor);
        self.apply_vim_action(action);
    }

    /// Apply a VimAction to the input buffer.
    fn apply_vim_action(&mut self, action: VimAction) {
        match action {
            VimAction::None => {}
            VimAction::InsertChar(ch) => {
                self.input.insert(self.input_cursor, ch);
                self.input_cursor += ch.len_utf8();
            }
            VimAction::DeleteChar => {
                if self.input_cursor > 0 {
                    let prev = prev_char_boundary(&self.input, self.input_cursor);
                    self.input.drain(prev..self.input_cursor);
                    self.input_cursor = prev;
                }
            }
            VimAction::DeleteRange(start, end) => {
                let s = start.min(self.input.len());
                let e = end.min(self.input.len());
                if s < e {
                    self.input.drain(s..e);
                    self.input_cursor = s.min(self.input.len());
                }
            }
            VimAction::MoveCursor(pos) => {
                self.input_cursor = pos.min(self.input.len());
            }
            VimAction::ChangeToInsert => {
                // The delete was already handled; vim switched mode.
            }
            VimAction::Yank(_text) => {
                // Already stored in vim.register.
            }
            VimAction::Paste(text) => {
                let pos = self.input_cursor.min(self.input.len());
                self.input.insert_str(pos, &text);
                self.input_cursor = pos + text.len();
            }
            VimAction::SwitchToNormal | VimAction::SwitchToInsert => {}
            VimAction::NewLine => {
                self.submit_message();
            }
        }
    }

    // ── Input history ─────────────────────────────────────────────

    fn history_prev(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        match self.history_index {
            None => {
                // Save current input, switch to the most recent history entry.
                self.history_stash = self.input.clone();
                self.history_index = Some(self.input_history.len() - 1);
            }
            Some(idx) if idx > 0 => {
                self.history_index = Some(idx - 1);
            }
            _ => return,
        }
        if let Some(idx) = self.history_index {
            self.input = self.input_history[idx].clone();
            self.input_cursor = self.input.len();
        }
    }

    fn history_next(&mut self) {
        match self.history_index {
            None => return,
            Some(idx) => {
                if idx + 1 < self.input_history.len() {
                    self.history_index = Some(idx + 1);
                    self.input = self.input_history[idx + 1].clone();
                    self.input_cursor = self.input.len();
                } else {
                    // Return to the stashed input.
                    self.history_index = None;
                    self.input = std::mem::take(&mut self.history_stash);
                    self.input_cursor = self.input.len();
                }
            }
        }
    }

    // ── Submit message ────────────────────────────────────────────

    /// Submit the current input as a user message.
    fn submit_message(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() {
            return;
        }

        // Handle built-in slash commands.
        if text == "/exit" || text == "/quit" {
            self.mode = AppMode::Exiting;
            return;
        }
        if text == "/clear" {
            self.messages.clear();
            self.input.clear();
            self.input_cursor = 0;
            return;
        }
        if text == "/help" {
            self.show_help = true;
            self.input.clear();
            self.input_cursor = 0;
            return;
        }

        // Save to history (avoid duplicates of the last entry).
        if self.input_history.last().map(|s| s.as_str()) != Some(&text) {
            self.input_history.push(text.clone());
        }
        self.history_index = None;
        self.history_stash.clear();

        // Add the user message to the display.
        self.messages.push(DisplayMessage {
            role: "user".to_string(),
            content: text.clone(),
            timestamp: now_str(),
            tool_name: None,
        });

        // Clear input.
        self.input.clear();
        self.input_cursor = 0;
        self.scroll_offset = 0;
        self.completion_candidates.clear();

        // Switch to processing mode.
        self.mode = AppMode::Processing;
        self.status = "Thinking...".to_string();
        self.current_assistant_text.clear();

        tracing::info!(message = %text, "user submitted message");
    }

    // ── Mouse scrolling ───────────────────────────────────────────

    /// Handle a mouse scroll event.
    pub fn handle_mouse_scroll(&mut self, up: bool) {
        if up {
            self.scroll_offset = self.scroll_offset.saturating_add(3);
        } else {
            self.scroll_offset = self.scroll_offset.saturating_sub(3);
        }
    }

    // ── Query event handling ──────────────────────────────────────

    /// Handle a query event from the agentic loop.
    pub fn handle_query_event(&mut self, event: QueryEvent) {
        match event {
            QueryEvent::Text(text) => {
                self.current_assistant_text.push_str(&text);
                self.status = "Responding...".to_string();
            }
            QueryEvent::Thinking(text) => {
                self.messages.push(DisplayMessage {
                    role: "thinking".to_string(),
                    content: text,
                    timestamp: now_str(),
                    tool_name: None,
                });
            }
            QueryEvent::ToolUseStart { name, id: _ } => {
                self.status = format!("Running {}...", name);
                self.messages.push(DisplayMessage {
                    role: "tool".to_string(),
                    content: format!("Using tool: {}", name),
                    timestamp: now_str(),
                    tool_name: Some(name),
                });
            }
            QueryEvent::ToolResult {
                id: _,
                output,
                is_error,
            } => {
                let role = if is_error { "error" } else { "tool" };
                self.messages.push(DisplayMessage {
                    role: role.to_string(),
                    content: output,
                    timestamp: now_str(),
                    tool_name: None,
                });
            }
            QueryEvent::TurnComplete {
                stop_reason: _,
                input_tokens,
                output_tokens,
            } => {
                self.turn_count += 1;
                self.total_tokens += input_tokens + output_tokens;
                // Flush accumulated assistant text as a message.
                if !self.current_assistant_text.is_empty() {
                    let text = std::mem::take(&mut self.current_assistant_text);
                    self.messages.push(DisplayMessage {
                        role: "assistant".to_string(),
                        content: text,
                        timestamp: now_str(),
                        tool_name: None,
                    });
                }
                tracing::debug!(
                    input_tokens,
                    output_tokens,
                    turn = self.turn_count,
                    "turn complete"
                );
            }
            QueryEvent::Error(msg) => {
                self.messages.push(DisplayMessage {
                    role: "error".to_string(),
                    content: msg,
                    timestamp: now_str(),
                    tool_name: None,
                });
            }
            QueryEvent::Done {
                total_turns: _,
                total_cost,
            } => {
                self.cost_display = total_cost;
                self.mode = AppMode::Input;
                self.status = "Ready".to_string();
                if !self.current_assistant_text.is_empty() {
                    let text = std::mem::take(&mut self.current_assistant_text);
                    self.messages.push(DisplayMessage {
                        role: "assistant".to_string(),
                        content: text,
                        timestamp: now_str(),
                        tool_name: None,
                    });
                }
            }
        }
    }

    // ── Scrolling ─────────────────────────────────────────────────

    /// Scroll the message area up by one page.
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(5);
    }

    /// Scroll the message area down (toward most recent).
    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(5);
    }

    // ── Helpers ───────────────────────────────────────────────────

    /// Returns the Vim mode indicator string for the status bar.
    pub fn vim_mode_str(&self) -> &str {
        if !self.vim_enabled {
            return "";
        }
        match self.vim.mode {
            VimMode::Insert => "-- INSERT --",
            VimMode::Normal => "-- NORMAL --",
        }
    }

    /// Search query getter for the renderer.
    pub fn active_search_term(&self) -> Option<&str> {
        if self.search_mode && !self.search_query.is_empty() {
            Some(&self.search_query)
        } else {
            None
        }
    }
}

// ── Utility functions ──────────────────────────────────────────────

fn now_str() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn next_char_boundary(text: &str, pos: usize) -> usize {
    let mut p = pos + 1;
    while p < text.len() && !text.is_char_boundary(p) {
        p += 1;
    }
    p.min(text.len())
}

fn prev_char_boundary(text: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut p = pos - 1;
    while p > 0 && !text.is_char_boundary(p) {
        p -= 1;
    }
    p
}
