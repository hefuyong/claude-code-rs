//! Color theme definitions for the TUI.
//!
//! Provides dark and light themes with role-based colors for every
//! element the UI renders: messages, diffs, search highlights, status
//! bar, borders, etc.

use ratatui::style::{Color, Modifier, Style};

/// Complete color palette for the TUI.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Style for user messages.
    pub user_msg: Color,
    /// Style for assistant messages.
    pub assistant_msg: Color,
    /// Style for system messages.
    pub system_msg: Color,
    /// Style for tool-use messages.
    pub tool_use: Color,
    /// Style for tool-result messages.
    pub tool_result: Color,
    /// Style for error messages.
    pub error: Color,
    /// Style for thinking/reasoning text.
    pub thinking: Color,
    /// Color for diff additions (`+` lines).
    pub diff_add: Color,
    /// Color for diff removals (`-` lines).
    pub diff_remove: Color,
    /// Background color for search-match highlights.
    pub search_highlight: Color,
    /// Style for the bottom status bar.
    pub status_bar: Style,
    /// Style for the top title bar.
    pub title_bar: Style,
    /// Style for widget borders.
    pub border: Style,
    /// Style for the cursor block.
    pub cursor: Style,
    /// Background color for modal dialogs.
    pub dialog_bg: Color,
    /// Foreground color for modal dialog text.
    pub dialog_fg: Color,
    /// Background for the focused button in dialogs.
    pub dialog_button_active: Color,
    /// Color for spinner frames.
    pub spinner: Color,
    /// Foreground color for the task panel header.
    pub task_header: Color,
    /// Color for running-task entries.
    pub task_running: Color,
    /// Color for completed-task entries.
    pub task_completed: Color,
    /// Color for pending-task entries.
    pub task_pending: Color,
}

impl Theme {
    /// Dark theme (default) -- light text on dark backgrounds.
    pub fn dark() -> Self {
        Self {
            user_msg: Color::Cyan,
            assistant_msg: Color::White,
            system_msg: Color::Magenta,
            tool_use: Color::Yellow,
            tool_result: Color::Yellow,
            error: Color::Red,
            thinking: Color::DarkGray,
            diff_add: Color::Green,
            diff_remove: Color::Red,
            search_highlight: Color::Yellow,
            status_bar: Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
            title_bar: Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
            border: Style::default().fg(Color::DarkGray),
            cursor: Style::default().fg(Color::Black).bg(Color::White),
            dialog_bg: Color::DarkGray,
            dialog_fg: Color::White,
            dialog_button_active: Color::Cyan,
            spinner: Color::Cyan,
            task_header: Color::Cyan,
            task_running: Color::Yellow,
            task_completed: Color::Green,
            task_pending: Color::DarkGray,
        }
    }

    /// Light theme -- dark text on light backgrounds.
    pub fn light() -> Self {
        Self {
            user_msg: Color::Blue,
            assistant_msg: Color::Black,
            system_msg: Color::Magenta,
            tool_use: Color::Rgb(180, 120, 0),
            tool_result: Color::Rgb(180, 120, 0),
            error: Color::Red,
            thinking: Color::Gray,
            diff_add: Color::Rgb(0, 128, 0),
            diff_remove: Color::Rgb(180, 0, 0),
            search_highlight: Color::Rgb(255, 200, 0),
            status_bar: Style::default()
                .fg(Color::Black)
                .bg(Color::Gray)
                .add_modifier(Modifier::BOLD),
            title_bar: Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
            border: Style::default().fg(Color::Gray),
            cursor: Style::default().fg(Color::White).bg(Color::Black),
            dialog_bg: Color::Gray,
            dialog_fg: Color::Black,
            dialog_button_active: Color::Blue,
            spinner: Color::Blue,
            task_header: Color::Blue,
            task_running: Color::Rgb(180, 120, 0),
            task_completed: Color::Rgb(0, 128, 0),
            task_pending: Color::Gray,
        }
    }

    /// Get the foreground color for a message role.
    pub fn role_color(&self, role: &str) -> Color {
        match role {
            "user" => self.user_msg,
            "assistant" => self.assistant_msg,
            "system" => self.system_msg,
            "tool" => self.tool_use,
            "error" => self.error,
            "thinking" => self.thinking,
            _ => self.assistant_msg,
        }
    }

    /// Get the role style (color + modifiers) for the role label.
    pub fn role_style(&self, role: &str) -> Style {
        let base = Style::default().fg(self.role_color(role));
        match role {
            "thinking" => base.add_modifier(Modifier::DIM),
            _ => base.add_modifier(Modifier::BOLD),
        }
    }

    /// Get the content style for a message role.
    pub fn content_style(&self, role: &str) -> Style {
        let base = Style::default().fg(self.role_color(role));
        if role == "thinking" {
            base.add_modifier(Modifier::DIM)
        } else {
            base
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}
