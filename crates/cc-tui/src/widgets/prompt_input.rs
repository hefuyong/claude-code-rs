//! Prompt input widget.
//!
//! Renders the multi-line input area with a cursor indicator, a prompt
//! prefix ("> " or "... " during processing), and an optional Vim mode
//! tag.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Widget};

/// The input prompt widget.
pub struct PromptInputWidget<'a> {
    /// The full input text (may contain newlines for multi-line).
    text: &'a str,
    /// Byte-offset cursor position in `text`.
    cursor: usize,
    /// Whether the app is currently processing a query.
    processing: bool,
    /// Vim mode indicator (empty string if vim mode is off).
    vim_indicator: &'a str,
    /// Cursor style.
    cursor_style: Style,
    /// Optional block border.
    block: Option<Block<'a>>,
}

impl<'a> PromptInputWidget<'a> {
    pub fn new(text: &'a str, cursor: usize) -> Self {
        Self {
            text,
            cursor,
            processing: false,
            vim_indicator: "",
            cursor_style: Style::default().fg(Color::Black).bg(Color::White),
            block: None,
        }
    }

    pub fn processing(mut self, p: bool) -> Self {
        self.processing = p;
        self
    }

    pub fn vim_indicator(mut self, ind: &'a str) -> Self {
        self.vim_indicator = ind;
        self
    }

    pub fn cursor_style(mut self, s: Style) -> Self {
        self.cursor_style = s;
        self
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }
}

impl Widget for PromptInputWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let inner = if let Some(ref block) = self.block {
            let inner = block.inner(area);
            block.clone().render(area, buf);
            inner
        } else {
            area
        };

        if inner.width < 4 || inner.height == 0 {
            return;
        }

        let prompt = if self.processing { "... " } else { "> " };

        // Split text around cursor.
        let cursor_pos = self.cursor.min(self.text.len());
        let before = &self.text[..cursor_pos];
        let after = &self.text[cursor_pos..];

        let cursor_char = after.chars().next().unwrap_or(' ');
        let after_cursor = if after.is_empty() {
            ""
        } else {
            let len = cursor_char.len_utf8();
            &after[len..]
        };

        // Render the first visible line of input.
        let mut spans = vec![
            Span::styled(prompt, Style::default().fg(Color::Green)),
        ];

        // For multi-line, only show the last line that fits the cursor.
        let input_lines: Vec<&str> = before.split('\n').collect();
        let current_line_before = input_lines.last().copied().unwrap_or("");

        // Show lines above the cursor line if we have room.
        let total_input_lines = self.text.split('\n').count();
        let cursor_line_idx = input_lines.len().saturating_sub(1);

        if total_input_lines > 1 && inner.height > 1 {
            // Multi-line: render previous lines first.
            let all_input_lines: Vec<&str> = self.text.split('\n').collect();
            let visible_start = cursor_line_idx.saturating_sub(inner.height as usize - 1);
            for (i, &line) in all_input_lines[visible_start..cursor_line_idx]
                .iter()
                .enumerate()
            {
                let y = inner.y + i as u16;
                if y >= inner.y + inner.height {
                    break;
                }
                let l = Line::from(vec![
                    Span::styled(prompt, Style::default().fg(Color::Green)),
                    Span::raw(line),
                ]);
                buf.set_line(inner.x, y, &l, inner.width);
            }
            // The cursor line goes on the next row.
            let cursor_y = inner.y
                + (cursor_line_idx - visible_start).min(inner.height as usize - 1) as u16;
            spans.push(Span::raw(current_line_before));
            spans.push(Span::styled(cursor_char.to_string(), self.cursor_style));
            let after_newline = after_cursor.split('\n').next().unwrap_or("");
            spans.push(Span::raw(after_newline));
            if !self.vim_indicator.is_empty() {
                spans.push(Span::styled(
                    format!("  {}", self.vim_indicator),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            buf.set_line(inner.x, cursor_y, &Line::from(spans), inner.width);
        } else {
            // Single-line rendering.
            spans.push(Span::raw(current_line_before));
            spans.push(Span::styled(cursor_char.to_string(), self.cursor_style));
            spans.push(Span::raw(after_cursor));
            if !self.vim_indicator.is_empty() {
                spans.push(Span::styled(
                    format!("  {}", self.vim_indicator),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
            }
            buf.set_line(inner.x, inner.y, &Line::from(spans), inner.width);
        }
    }
}
