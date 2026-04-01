//! Animated spinner widget.
//!
//! Renders a single-line spinner with a configurable set of animation
//! frames and an associated label. The caller advances the frame index
//! on each tick.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

/// Braille-dot spinner frames.
const SPINNER_FRAMES: &[&str] = &[
    "\u{28f7}", // ⣷
    "\u{28ef}", // ⣯
    "\u{28df}", // ⣟
    "\u{287f}", // ⡿
    "\u{28bf}", // ⢿
    "\u{28fb}", // ⣻
    "\u{28fd}", // ⣽
    "\u{28fe}", // ⣾
];

/// A single-line spinner with a label.
pub struct SpinnerWidget<'a> {
    /// The text displayed after the spinner glyph.
    label: &'a str,
    /// Current frame index (mod SPINNER_FRAMES.len()).
    frame: usize,
    /// Style for the spinner glyph.
    spinner_style: Style,
    /// Style for the label text.
    label_style: Style,
}

impl<'a> SpinnerWidget<'a> {
    pub fn new(label: &'a str, frame: usize) -> Self {
        Self {
            label,
            frame,
            spinner_style: Style::default().fg(Color::Cyan),
            label_style: Style::default().fg(Color::Yellow),
        }
    }

    pub fn spinner_style(mut self, style: Style) -> Self {
        self.spinner_style = style;
        self
    }

    pub fn label_style(mut self, style: Style) -> Self {
        self.label_style = style;
        self
    }
}

impl Widget for SpinnerWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 4 || area.height == 0 {
            return;
        }
        let idx = self.frame % SPINNER_FRAMES.len();
        let glyph = SPINNER_FRAMES[idx];

        let line = Line::from(vec![
            Span::styled(format!("{glyph} "), self.spinner_style),
            Span::styled(self.label, self.label_style),
        ]);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

/// Return the number of available spinner frames (useful for wrapping).
pub fn frame_count() -> usize {
    SPINNER_FRAMES.len()
}
