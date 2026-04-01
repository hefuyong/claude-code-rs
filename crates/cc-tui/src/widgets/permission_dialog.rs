//! Permission dialog widget.
//!
//! A centered modal overlay that asks the user to allow or deny a tool
//! execution. Renders a bordered box with the tool name, description,
//! and keybinding hints `[y]es / [n]o / [a]lways`.

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};

/// Data passed into the dialog.
pub struct PermissionDialogData<'a> {
    pub tool_name: &'a str,
    pub message: &'a str,
}

/// The dialog widget itself.
pub struct PermissionDialogWidget<'a> {
    data: PermissionDialogData<'a>,
    border_style: Style,
    bg: Color,
    fg: Color,
    button_active: Color,
}

impl<'a> PermissionDialogWidget<'a> {
    pub fn new(data: PermissionDialogData<'a>) -> Self {
        Self {
            data,
            border_style: Style::default().fg(Color::Red),
            bg: Color::DarkGray,
            fg: Color::White,
            button_active: Color::Cyan,
        }
    }

    pub fn border_style(mut self, s: Style) -> Self {
        self.border_style = s;
        self
    }

    pub fn colors(mut self, bg: Color, fg: Color, button_active: Color) -> Self {
        self.bg = bg;
        self.fg = fg;
        self.button_active = button_active;
        self
    }
}

impl Widget for PermissionDialogWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dialog = centered_rect(60, 40, area);

        // Clear the area behind the dialog.
        Clear.render(dialog, buf);

        let block = Block::default()
            .title(" Permission Required ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(self.border_style)
            .style(Style::default().bg(self.bg).fg(self.fg));

        let inner = block.inner(dialog);
        block.render(dialog, buf);

        // Split inner area: tool name (2), message (flex), buttons (2).
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(3),
                Constraint::Length(2),
            ])
            .split(inner);

        // Tool name header.
        let header = Paragraph::new(Line::from(vec![
            Span::styled("Tool: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                self.data.tool_name,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));
        header.render(chunks[0], buf);

        // Message body.
        let body = Paragraph::new(self.data.message)
            .style(Style::default().fg(self.fg))
            .wrap(Wrap { trim: false });
        body.render(chunks[1], buf);

        // Button hints.
        let buttons = Line::from(vec![
            Span::styled(
                " [y]es ",
                Style::default()
                    .fg(Color::Black)
                    .bg(self.button_active)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                " [n]o ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                " [a]lways ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
        let button_para = Paragraph::new(buttons).alignment(Alignment::Center);
        button_para.render(chunks[2], buf);
    }
}

/// Return a centered `Rect` that is `percent_x`% wide and `percent_y`% tall.
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
