//! Scrollable message list widget.
//!
//! Implements virtual scrolling -- only messages whose rendered lines
//! fall inside the visible viewport are formatted and written to the
//! buffer. This keeps the cost constant regardless of conversation
//! length.

use crate::app::DisplayMessage;
use crate::theme::Theme;
use ratatui::style::Color;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Widget};

/// A virtual-scrolling message list.
pub struct MessageListWidget<'a> {
    messages: &'a [DisplayMessage],
    /// Current streaming text (shown at bottom while processing).
    streaming_text: Option<&'a str>,
    /// Scroll offset: 0 = pinned to bottom; positive = scrolled up.
    scroll_offset: usize,
    /// Optional search term to highlight.
    search_term: Option<&'a str>,
    /// Theme for coloring.
    theme: &'a Theme,
    /// Block (border + title).
    block: Option<Block<'a>>,
}

impl<'a> MessageListWidget<'a> {
    pub fn new(messages: &'a [DisplayMessage], theme: &'a Theme) -> Self {
        Self {
            messages,
            streaming_text: None,
            scroll_offset: 0,
            search_term: None,
            theme,
            block: None,
        }
    }

    pub fn streaming_text(mut self, text: Option<&'a str>) -> Self {
        self.streaming_text = text;
        self
    }

    pub fn scroll_offset(mut self, offset: usize) -> Self {
        self.scroll_offset = offset;
        self
    }

    pub fn search_term(mut self, term: Option<&'a str>) -> Self {
        self.search_term = term;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl Widget for MessageListWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Resolve inner area (inside border).
        let inner = if let Some(ref block) = self.block {
            let inner = block.inner(area);
            block.clone().render(area, buf);
            inner
        } else {
            area
        };

        if inner.width < 2 || inner.height < 1 {
            return;
        }

        let visible_height = inner.height as usize;

        // Flatten all messages into Lines.
        let mut all_lines = Vec::with_capacity(self.messages.len() * 4);

        for msg in self.messages {
            render_message_lines(msg, self.theme, self.search_term, &mut all_lines);
            all_lines.push(Line::from(""));
        }

        // Append streaming text if present.
        if let Some(text) = self.streaming_text {
            if !text.is_empty() {
                all_lines.push(Line::from(Span::styled(
                    "assistant:",
                    Style::default()
                        .fg(self.theme.assistant_msg)
                        .add_modifier(Modifier::BOLD),
                )));
                for line in text.lines() {
                    all_lines.push(Line::from(Span::styled(
                        format!("  {line}"),
                        Style::default().fg(self.theme.assistant_msg),
                    )));
                }
            }
        }

        // Virtual scroll: compute the visible window.
        let total = all_lines.len();
        let scroll = self.scroll_offset.min(total.saturating_sub(visible_height));
        let end = total.saturating_sub(scroll);
        let start = end.saturating_sub(visible_height);

        for (i, line) in all_lines[start..end].iter().enumerate() {
            let y = inner.y + i as u16;
            if y >= inner.y + inner.height {
                break;
            }
            buf.set_line(inner.x, y, line, inner.width);
        }
    }
}

/// Convert a single `DisplayMessage` into a sequence of `Line`s.
fn render_message_lines<'a>(
    msg: &'a DisplayMessage,
    theme: &'a Theme,
    search_term: Option<&str>,
    out: &mut Vec<Line<'a>>,
) {
    let role_style = theme.role_style(&msg.role);
    let role_label = role_label(&msg.role);

    // Header line: [timestamp] role: (tool_name)
    let mut header_spans: Vec<Span> = vec![
        Span::styled(
            format!("[{}] ", msg.timestamp),
            Style::default().fg(theme.thinking),
        ),
        Span::styled(role_label, role_style),
    ];

    if let Some(ref tool) = msg.tool_name {
        header_spans.push(Span::styled(
            format!(" ({tool})"),
            Style::default().fg(theme.tool_use),
        ));
    }
    out.push(Line::from(header_spans));

    // Content lines, optionally with diff coloring or search highlighting.
    let content_style = theme.content_style(&msg.role);
    let is_diff = msg.content.lines().any(|l| {
        l.starts_with("+++")
            || l.starts_with("---")
            || (l.starts_with('+') && !l.starts_with("+++"))
            || (l.starts_with('-') && !l.starts_with("---"))
    });

    for line in msg.content.lines() {
        let styled = if is_diff {
            diff_line_style(line, theme)
        } else if let Some(term) = search_term {
            highlight_search(line, term, content_style, theme.search_highlight)
        } else {
            Line::from(Span::styled(format!("  {line}"), content_style))
        };
        out.push(styled);
    }
}

/// Apply diff coloring to a single line.
fn diff_line_style<'a>(line: &'a str, theme: &Theme) -> Line<'a> {
    let (color, prefix) = if line.starts_with("+++") || line.starts_with("---") {
        (theme.assistant_msg, "  ")
    } else if line.starts_with('+') {
        (theme.diff_add, "  ")
    } else if line.starts_with('-') {
        (theme.diff_remove, "  ")
    } else if line.starts_with("@@") {
        (theme.system_msg, "  ")
    } else {
        (theme.assistant_msg, "  ")
    };
    Line::from(Span::styled(
        format!("{prefix}{line}"),
        Style::default().fg(color),
    ))
}

/// Highlight occurrences of `term` in `line` with a background color.
fn highlight_search<'a>(
    line: &'a str,
    term: &str,
    base: Style,
    highlight_color: Color,
) -> Line<'a> {
    if term.is_empty() {
        return Line::from(Span::styled(format!("  {line}"), base));
    }

    let lower = line.to_lowercase();
    let term_lower = term.to_lowercase();
    let mut spans = Vec::new();
    spans.push(Span::styled("  ", base));

    let mut last = 0;
    for (idx, _) in lower.match_indices(&term_lower) {
        if idx > last {
            spans.push(Span::styled(&line[last..idx], base));
        }
        spans.push(Span::styled(
            &line[idx..idx + term.len()],
            base.bg(highlight_color),
        ));
        last = idx + term.len();
    }
    if last < line.len() {
        spans.push(Span::styled(&line[last..], base));
    }
    Line::from(spans)
}

/// Map a role string to a display label.
fn role_label(role: &str) -> &'static str {
    match role {
        "user" => "you:",
        "assistant" => "assistant:",
        "tool" => "tool:",
        "error" => "error:",
        "thinking" => "thinking:",
        "system" => "system:",
        _ => "unknown:",
    }
}
