//! UI rendering for the TUI.
//!
//! Uses ratatui widgets -- both built-in and from our `widgets` module
//! -- to render the full interface: title bar, message list (virtual
//! scrolling), input prompt, status bar, search bar, task panel, help
//! overlay, and permission dialog.

use crate::app::{App, AppMode};
use crate::theme::Theme;
use crate::widgets::message_list::MessageListWidget;
use crate::widgets::permission_dialog::{PermissionDialogData, PermissionDialogWidget};
use crate::widgets::prompt_input::PromptInputWidget;
use crate::widgets::spinner::SpinnerWidget;

use cc_tasks::TaskStatus;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

// ── Top-level render ───────────────────────────────────────────────

/// Render the full TUI layout onto the given frame.
pub fn render(f: &mut Frame, app: &App) {
    let size = f.area();

    // Determine the horizontal split depending on whether the task
    // panel is visible.
    let (main_area, task_area) = if app.show_task_panel {
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(40), Constraint::Length(30)])
            .split(size);
        (h_chunks[0], Some(h_chunks[1]))
    } else {
        (size, None)
    };

    // Vertical layout: title (3) | messages (flex) | spinner? (1) | search? (1) | input (3) | status (1)
    let has_spinner = matches!(app.mode, AppMode::Processing);
    let has_search = app.search_mode;

    let mut constraints = vec![
        Constraint::Length(3), // title bar
    ];
    constraints.push(Constraint::Min(5)); // messages
    if has_spinner {
        constraints.push(Constraint::Length(1)); // spinner row
    }
    if has_search {
        constraints.push(Constraint::Length(1)); // search bar
    }
    constraints.push(Constraint::Length(3)); // input
    constraints.push(Constraint::Length(1)); // status bar

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(main_area);

    let mut idx = 0;

    // Title bar.
    render_title_bar(f, app, chunks[idx]);
    idx += 1;

    // Message area.
    render_messages(f, app, chunks[idx]);
    idx += 1;

    // Spinner row (only during processing).
    if has_spinner {
        render_spinner(f, app, chunks[idx]);
        idx += 1;
    }

    // Search bar.
    if has_search {
        render_search_bar(f, app, chunks[idx]);
        idx += 1;
    }

    // Input area.
    render_input(f, app, chunks[idx]);
    idx += 1;

    // Status bar.
    render_status_bar(f, app, chunks[idx]);

    // Task panel (right side).
    if let Some(area) = task_area {
        render_task_panel(f, app, area);
    }

    // ── Overlays (rendered last, on top of everything) ─────────

    // Permission dialog.
    if let AppMode::PermissionPrompt(ref prompt) = app.mode {
        render_permission_dialog(f, prompt, size);
    }

    // Help overlay.
    if app.show_help {
        render_help_overlay(f, size);
    }
}

// ── Title bar ──────────────────────────────────────────────────────

fn render_title_bar(f: &mut Frame, app: &App, area: Rect) {
    let status_style = match &app.mode {
        AppMode::Processing => Style::default().fg(Color::Yellow),
        AppMode::PermissionPrompt(_) => Style::default().fg(Color::Red),
        _ => Style::default().fg(Color::Green),
    };

    let title_text = Line::from(vec![
        Span::styled(
            " Claude Code RS ",
            app.theme.title_bar,
        ),
        Span::raw(" | "),
        Span::styled(&app.model, Style::default().fg(Color::White)),
        Span::raw(" | "),
        Span::styled(&app.cost_display, Style::default().fg(Color::Green)),
        Span::raw(" | "),
        Span::styled(
            format!("turns: {}", app.turn_count),
            Style::default().fg(Color::White),
        ),
        Span::raw(" | "),
        Span::styled(&app.status, status_style),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border);
    let paragraph = Paragraph::new(title_text).block(block);
    f.render_widget(paragraph, area);
}

// ── Message area (virtual scrolling) ───────────────────────────────

fn render_messages(f: &mut Frame, app: &App, area: Rect) {
    let streaming = if matches!(app.mode, AppMode::Processing)
        && !app.current_assistant_text.is_empty()
    {
        Some(app.current_assistant_text.as_str())
    } else {
        None
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border)
        .title(" Messages ");

    let widget = MessageListWidget::new(&app.messages, &app.theme)
        .streaming_text(streaming)
        .scroll_offset(app.scroll_offset)
        .search_term(app.active_search_term())
        .block(block);

    f.render_widget(widget, area);
}

// ── Spinner row ────────────────────────────────────────────────────

fn render_spinner(f: &mut Frame, app: &App, area: Rect) {
    let label = &app.status;
    let spinner = SpinnerWidget::new(label, app.spinner_frame)
        .spinner_style(Style::default().fg(app.theme.spinner))
        .label_style(Style::default().fg(Color::Yellow));
    f.render_widget(spinner, area);
}

// ── Search bar ─────────────────────────────────────────────────────

fn render_search_bar(f: &mut Frame, app: &App, area: Rect) {
    let match_info = if app.search_matches.is_empty() {
        if app.search_query.is_empty() {
            String::new()
        } else {
            " (no matches)".to_string()
        }
    } else {
        format!(
            " ({}/{})",
            app.search_current + 1,
            app.search_matches.len()
        )
    };

    let line = Line::from(vec![
        Span::styled(
            " Search: ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(&app.search_query, Style::default().fg(Color::White)),
        Span::styled(
            match_info,
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            "  [Esc] close  [Enter/Ctrl+N] next  [Ctrl+P] prev",
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

// ── Input area ─────────────────────────────────────────────────────

fn render_input(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border)
        .title(" Input ");

    let widget = PromptInputWidget::new(&app.input, app.input_cursor)
        .processing(matches!(app.mode, AppMode::Processing))
        .vim_indicator(app.vim_mode_str())
        .cursor_style(app.theme.cursor)
        .block(block);

    f.render_widget(widget, area);
}

// ── Status bar ─────────────────────────────────────────────────────

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let vim_indicator = app.vim_mode_str();

    let mut spans: Vec<Span> = Vec::new();

    // Vim mode badge.
    if !vim_indicator.is_empty() {
        let (bg, label) = match app.vim.mode {
            cc_vim::VimMode::Normal => (Color::Blue, " NORMAL "),
            cc_vim::VimMode::Insert => (Color::Green, " INSERT "),
        };
        spans.push(Span::styled(
            label,
            Style::default()
                .fg(Color::Black)
                .bg(bg)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
    }

    // Model name.
    spans.push(Span::styled(
        &app.model,
        Style::default().fg(Color::Cyan),
    ));
    spans.push(Span::raw(" | "));

    // Token count.
    spans.push(Span::styled(
        format!("tokens: {}", format_tokens(app.total_tokens)),
        Style::default().fg(Color::White),
    ));
    spans.push(Span::raw(" | "));

    // Cost.
    spans.push(Span::styled(
        &app.cost_display,
        Style::default().fg(Color::Green),
    ));
    spans.push(Span::raw(" | "));

    // Turn count.
    spans.push(Span::styled(
        format!("turns: {}", app.turn_count),
        Style::default().fg(Color::White),
    ));

    // Right-aligned hints.
    let left_len: usize = spans.iter().map(|s| s.content.len()).sum();
    let right_hints = " F1:help  Tab:tasks  Ctrl+F:search  Ctrl+C:quit ";
    let padding = (area.width as usize)
        .saturating_sub(left_len)
        .saturating_sub(right_hints.len());
    spans.push(Span::raw(" ".repeat(padding)));
    spans.push(Span::styled(
        right_hints,
        Style::default().fg(Color::DarkGray),
    ));

    let status_line = Paragraph::new(Line::from(spans))
        .style(app.theme.status_bar);
    f.render_widget(status_line, area);
}

// ── Task panel ─────────────────────────────────────────────────────

fn render_task_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(app.theme.border)
        .title(Span::styled(
            " Tasks ",
            Style::default()
                .fg(app.theme.task_header)
                .add_modifier(Modifier::BOLD),
        ));

    if app.tasks.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "  No active tasks",
            Style::default().fg(Color::DarkGray),
        )))
        .block(block);
        f.render_widget(empty, area);
        return;
    }

    let inner = block.inner(area);

    let mut lines: Vec<Line> = Vec::new();
    for task in &app.tasks {
        let (icon, color) = match task.status {
            TaskStatus::Running => ("\u{25b6}", app.theme.task_running),    // ▶
            TaskStatus::Completed => ("\u{2714}", app.theme.task_completed), // ✔
            TaskStatus::Failed => ("\u{2718}", app.theme.error),             // ✘
            TaskStatus::Killed => ("\u{2718}", app.theme.error),
            TaskStatus::Pending => ("\u{25cb}", app.theme.task_pending),     // ○
        };
        let desc = task
            .description
            .as_deref()
            .unwrap_or(&task.id);
        lines.push(Line::from(vec![
            Span::styled(format!(" {icon} "), Style::default().fg(color)),
            Span::styled(
                truncate_str(desc, inner.width.saturating_sub(5) as usize),
                Style::default().fg(color),
            ),
        ]));
    }

    f.render_widget(block, area);
    let para = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(para, inner);
}

// ── Permission dialog ──────────────────────────────────────────────

fn render_permission_dialog(
    f: &mut Frame,
    prompt: &crate::app::PermPrompt,
    area: Rect,
) {
    let data = PermissionDialogData {
        tool_name: &prompt.tool_name,
        message: &prompt.message,
    };
    let widget = PermissionDialogWidget::new(data)
        .border_style(Style::default().fg(Color::Red));
    f.render_widget(widget, area);
}

// ── Help overlay ───────────────────────────────────────────────────

fn render_help_overlay(f: &mut Frame, area: Rect) {
    let dialog = centered_rect(70, 70, area);

    // Clear the area behind.
    f.render_widget(Clear, dialog);

    let block = Block::default()
        .title(" Keybindings (F1 to close) ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    let help_text = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  General",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from("  Enter         Send message"),
        Line::from("  Shift+Enter   Insert newline"),
        Line::from("  Ctrl+C        Quit"),
        Line::from("  Ctrl+D        Quit"),
        Line::from("  F1            Toggle this help"),
        Line::from(""),
        Line::from(Span::styled(
            "  Navigation",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from("  PageUp/Dn     Scroll messages"),
        Line::from("  Mouse scroll   Scroll messages"),
        Line::from("  Up/Down       Input history"),
        Line::from(""),
        Line::from(Span::styled(
            "  Features",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from("  Ctrl+F        Search messages"),
        Line::from("  Tab           Toggle task panel / complete command"),
        Line::from(""),
        Line::from(Span::styled(
            "  Search Mode",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from("  Esc           Exit search"),
        Line::from("  Enter/Ctrl+N  Next match"),
        Line::from("  Ctrl+P        Previous match"),
        Line::from(""),
        Line::from(Span::styled(
            "  Permission Dialog",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from("  y             Allow once"),
        Line::from("  n / Esc       Deny"),
        Line::from("  a             Always allow"),
        Line::from(""),
        Line::from(Span::styled(
            "  Vim Mode (when enabled)",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from("  Esc           Normal mode"),
        Line::from("  i / a / o     Insert mode"),
        Line::from("  h / l         Move left / right"),
        Line::from("  w / b / e     Word motions"),
        Line::from("  d{motion}     Delete"),
        Line::from("  c{motion}     Change"),
        Line::from("  y{motion}     Yank"),
        Line::from("  p / P         Paste after / before"),
        Line::from("  dd / cc / yy  Line operations"),
        Line::from(""),
        Line::from(Span::styled(
            "  Slash Commands",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )),
        Line::from("  /clear        Clear messages"),
        Line::from("  /exit         Quit"),
        Line::from("  /help         Show help"),
        Line::from("  /compact      Compact conversation"),
        Line::from("  /cost         Show cost summary"),
        Line::from("  /model        Switch model"),
        Line::from(""),
    ];

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, dialog);
}

// ── Diff rendering helper ──────────────────────────────────────────

/// Render a diff text block with green/red coloring inside the given
/// area. This is a standalone helper for when you need to render diff
/// content outside of the message list.
pub fn render_diff(f: &mut Frame, text: &str, area: Rect, theme: &Theme) {
    let mut lines: Vec<Line> = Vec::new();
    for line in text.lines() {
        let (color, _prefix) = if line.starts_with("+++") || line.starts_with("---") {
            (theme.assistant_msg, "")
        } else if line.starts_with('+') {
            (theme.diff_add, "")
        } else if line.starts_with('-') {
            (theme.diff_remove, "")
        } else if line.starts_with("@@") {
            (theme.system_msg, "")
        } else {
            (theme.assistant_msg, "")
        };
        lines.push(Line::from(Span::styled(
            line,
            Style::default().fg(color),
        )));
    }
    let para = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

// ── Layout helpers ─────────────────────────────────────────────────

/// Return a centered `Rect` that is `percent_x`% wide and `percent_y`%
/// tall relative to `area`.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vert[1])[1]
}

/// Truncate a string to at most `max_len` characters, appending "..."
/// if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 3 {
        format!("{}...", &s[..max_len - 3])
    } else {
        s[..max_len].to_string()
    }
}

/// Format a token count with `k` / `M` suffixes.
fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
