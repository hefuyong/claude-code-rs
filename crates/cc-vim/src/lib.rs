//! Vim mode state machine for the Claude Code RS input line.
//!
//! Implements a minimal but useful subset of Vim keybindings: insert
//! and normal mode, motions (h/l/w/b/e/0/$), operators (d/c/y),
//! line operations (dd/cc/yy), find (f/F/t/T), paste (p/P), and more.

use serde::{Deserialize, Serialize};

// ── Public types ────────────────────────────────────────────────────

/// The two modes of the Vim state machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VimMode {
    Insert,
    Normal,
}

/// The pending command state while in Normal mode.
#[derive(Debug, Clone)]
pub enum NormalCommand {
    Idle,
    Count(String),
    Operator(Operator),
    OperatorCount(Operator, String),
    Find(FindDir),
    Replace,
    G,
}

/// Operators that act on a motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
}

/// Direction for the f/F/t/T find commands.
#[derive(Debug, Clone, Copy)]
pub enum FindDir {
    Forward,
    Backward,
    ForwardTill,
    BackwardTill,
}

/// An action that the caller should apply to the text buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VimAction {
    /// Nothing to do.
    None,
    /// Insert a character at the cursor.
    InsertChar(char),
    /// Delete the character before the cursor (backspace).
    DeleteChar,
    /// Delete the range `[start, end)` in the text buffer.
    DeleteRange(usize, usize),
    /// Move the cursor to an absolute byte position.
    MoveCursor(usize),
    /// A change operation: delete the range, then enter insert mode.
    ChangeToInsert,
    /// Yank (copy) the given string into the register.
    Yank(String),
    /// Paste the given string at the cursor.
    Paste(String),
    /// Switch to normal mode.
    SwitchToNormal,
    /// Switch to insert mode.
    SwitchToInsert,
    /// Submit the current line (Enter in insert mode).
    NewLine,
}

/// The full Vim state machine.
pub struct VimState {
    /// Current mode.
    pub mode: VimMode,
    /// The pending command being assembled in normal mode.
    pub command: NormalCommand,
    /// The yank register.
    pub register: String,
    /// The last find command for `;` / `,` repeat.
    pub last_find: Option<(FindDir, char)>,
    /// The last simple change for `.` repeat (stored as a char sequence).
    last_change: Option<Vec<char>>,
}

impl Default for VimState {
    fn default() -> Self {
        Self::new()
    }
}

impl VimState {
    /// Create a new Vim state, starting in Insert mode.
    pub fn new() -> Self {
        Self {
            mode: VimMode::Insert,
            command: NormalCommand::Idle,
            register: String::new(),
            last_find: None,
            last_change: None,
        }
    }

    /// Process a key event and return the action the caller should apply.
    ///
    /// `text` is the current buffer content and `cursor` is the byte
    /// offset of the cursor within that text.
    pub fn handle_key(&mut self, key: char, text: &str, cursor: usize) -> VimAction {
        match self.mode {
            VimMode::Insert => self.handle_insert(key, text, cursor),
            VimMode::Normal => self.handle_normal(key, text, cursor),
        }
    }

    // ── Insert mode ─────────────────────────────────────────────────

    fn handle_insert(&mut self, key: char, _text: &str, _cursor: usize) -> VimAction {
        match key {
            '\x1b' => {
                // Escape -> Normal
                self.mode = VimMode::Normal;
                self.command = NormalCommand::Idle;
                VimAction::SwitchToNormal
            }
            '\n' | '\r' => VimAction::NewLine,
            '\x7f' | '\x08' => VimAction::DeleteChar, // backspace
            _ => VimAction::InsertChar(key),
        }
    }

    // ── Normal mode ─────────────────────────────────────────────────

    fn handle_normal(&mut self, key: char, text: &str, cursor: usize) -> VimAction {
        // Handle pending states first.
        match &self.command {
            NormalCommand::Find(dir) => {
                let dir = *dir;
                self.command = NormalCommand::Idle;
                return self.do_find(dir, key, text, cursor);
            }
            NormalCommand::Replace => {
                self.command = NormalCommand::Idle;
                return self.do_replace(key, text, cursor);
            }
            NormalCommand::G => {
                self.command = NormalCommand::Idle;
                if key == 'g' {
                    // gg -> go to start
                    return VimAction::MoveCursor(0);
                }
                return VimAction::None;
            }
            _ => {}
        }

        // Count prefix.
        if key.is_ascii_digit() && key != '0' {
            match &self.command {
                NormalCommand::Idle => {
                    self.command = NormalCommand::Count(key.to_string());
                    return VimAction::None;
                }
                NormalCommand::Count(s) => {
                    let mut s = s.clone();
                    s.push(key);
                    self.command = NormalCommand::Count(s);
                    return VimAction::None;
                }
                NormalCommand::Operator(op) => {
                    let op = *op;
                    self.command = NormalCommand::OperatorCount(op, key.to_string());
                    return VimAction::None;
                }
                NormalCommand::OperatorCount(op, s) => {
                    let op = *op;
                    let mut s = s.clone();
                    s.push(key);
                    self.command = NormalCommand::OperatorCount(op, s);
                    return VimAction::None;
                }
                _ => {}
            }
        }

        // Extract count and pending operator.
        let (count, pending_op) = self.extract_count_and_op();

        // Operator starters.
        match key {
            'd' if pending_op.is_none() => {
                self.command = NormalCommand::Operator(Operator::Delete);
                return VimAction::None;
            }
            'c' if pending_op.is_none() => {
                self.command = NormalCommand::Operator(Operator::Change);
                return VimAction::None;
            }
            'y' if pending_op.is_none() => {
                self.command = NormalCommand::Operator(Operator::Yank);
                return VimAction::None;
            }
            _ => {}
        }

        // Line operations: dd, cc, yy.
        if let Some(op) = pending_op {
            match (op, key) {
                (Operator::Delete, 'd') => {
                    self.command = NormalCommand::Idle;
                    let yanked = text.to_string();
                    self.register = yanked.clone();
                    return VimAction::DeleteRange(0, text.len());
                }
                (Operator::Change, 'c') => {
                    self.command = NormalCommand::Idle;
                    self.register = text.to_string();
                    self.mode = VimMode::Insert;
                    return VimAction::DeleteRange(0, text.len());
                }
                (Operator::Yank, 'y') => {
                    self.command = NormalCommand::Idle;
                    self.register = text.to_string();
                    return VimAction::Yank(text.to_string());
                }
                _ => {
                    // Operator + motion.
                    if let Some(target) = self.resolve_motion(key, text, cursor, count) {
                        self.command = NormalCommand::Idle;
                        let (start, end) = if target > cursor {
                            (cursor, target)
                        } else {
                            (target, cursor)
                        };
                        let region = &text[start..end];
                        self.register = region.to_string();
                        match op {
                            Operator::Delete => return VimAction::DeleteRange(start, end),
                            Operator::Change => {
                                self.mode = VimMode::Insert;
                                return VimAction::DeleteRange(start, end);
                            }
                            Operator::Yank => {
                                return VimAction::Yank(region.to_string());
                            }
                        }
                    } else {
                        // Unknown motion, cancel.
                        self.command = NormalCommand::Idle;
                        return VimAction::None;
                    }
                }
            }
        }

        self.command = NormalCommand::Idle;

        // Standalone commands.
        match key {
            // Mode switching
            'i' => {
                self.mode = VimMode::Insert;
                VimAction::SwitchToInsert
            }
            'a' => {
                self.mode = VimMode::Insert;
                let new_pos = next_char_boundary(text, cursor);
                VimAction::MoveCursor(new_pos)
            }
            'A' => {
                self.mode = VimMode::Insert;
                VimAction::MoveCursor(text.len())
            }
            'I' => {
                self.mode = VimMode::Insert;
                VimAction::MoveCursor(0)
            }

            // Motions
            'h' => {
                let new = move_left(text, cursor, count);
                VimAction::MoveCursor(new)
            }
            'l' => {
                let new = move_right(text, cursor, count);
                VimAction::MoveCursor(new)
            }
            'w' => {
                let new = word_forward(text, cursor, count);
                VimAction::MoveCursor(new)
            }
            'b' => {
                let new = word_backward(text, cursor, count);
                VimAction::MoveCursor(new)
            }
            'e' => {
                let new = word_end(text, cursor, count);
                VimAction::MoveCursor(new)
            }
            '0' => VimAction::MoveCursor(0),
            '$' => VimAction::MoveCursor(text.len().saturating_sub(1).max(0)),
            'G' => {
                // G -> go to end
                VimAction::MoveCursor(text.len().saturating_sub(1).max(0))
            }
            'g' => {
                self.command = NormalCommand::G;
                VimAction::None
            }

            // Delete char under cursor
            'x' => {
                if cursor < text.len() {
                    let end = next_char_boundary(text, cursor);
                    let ch = &text[cursor..end];
                    self.register = ch.to_string();
                    VimAction::DeleteRange(cursor, end)
                } else {
                    VimAction::None
                }
            }

            // Paste
            'p' => {
                if !self.register.is_empty() {
                    VimAction::Paste(self.register.clone())
                } else {
                    VimAction::None
                }
            }
            'P' => {
                if !self.register.is_empty() {
                    VimAction::Paste(self.register.clone())
                } else {
                    VimAction::None
                }
            }

            // Find
            'f' => {
                self.command = NormalCommand::Find(FindDir::Forward);
                VimAction::None
            }
            'F' => {
                self.command = NormalCommand::Find(FindDir::Backward);
                VimAction::None
            }
            't' => {
                self.command = NormalCommand::Find(FindDir::ForwardTill);
                VimAction::None
            }
            'T' => {
                self.command = NormalCommand::Find(FindDir::BackwardTill);
                VimAction::None
            }

            // Repeat last find
            ';' => {
                if let Some((dir, ch)) = self.last_find {
                    self.do_find(dir, ch, text, cursor)
                } else {
                    VimAction::None
                }
            }

            // Replace single char
            'r' => {
                self.command = NormalCommand::Replace;
                VimAction::None
            }

            // Dot repeat (simplified: re-paste from register)
            '.' => {
                if !self.register.is_empty() {
                    VimAction::Paste(self.register.clone())
                } else {
                    VimAction::None
                }
            }

            _ => VimAction::None,
        }
    }

    // ── Helpers ─────────────────────────────────────────────────────

    fn extract_count_and_op(&mut self) -> (usize, Option<Operator>) {
        let (count, op) = match &self.command {
            NormalCommand::Idle => (1, None),
            NormalCommand::Count(s) => (s.parse::<usize>().unwrap_or(1), None),
            NormalCommand::Operator(op) => (1, Some(*op)),
            NormalCommand::OperatorCount(op, s) => {
                (s.parse::<usize>().unwrap_or(1), Some(*op))
            }
            _ => (1, None),
        };
        (count, op)
    }

    fn resolve_motion(
        &self,
        key: char,
        text: &str,
        cursor: usize,
        count: usize,
    ) -> Option<usize> {
        match key {
            'h' => Some(move_left(text, cursor, count)),
            'l' => Some(move_right(text, cursor, count)),
            'w' => Some(word_forward(text, cursor, count)),
            'b' => Some(word_backward(text, cursor, count)),
            'e' => Some(next_char_boundary(text, word_end(text, cursor, count))),
            '0' => Some(0),
            '$' => Some(text.len()),
            _ => None,
        }
    }

    fn do_find(
        &mut self,
        dir: FindDir,
        ch: char,
        text: &str,
        cursor: usize,
    ) -> VimAction {
        self.last_find = Some((dir, ch));
        let chars: Vec<char> = text.chars().collect();
        let char_idx = text[..cursor].chars().count();

        match dir {
            FindDir::Forward => {
                for i in (char_idx + 1)..chars.len() {
                    if chars[i] == ch {
                        let byte_pos = char_to_byte(text, i);
                        return VimAction::MoveCursor(byte_pos);
                    }
                }
            }
            FindDir::Backward => {
                for i in (0..char_idx).rev() {
                    if chars[i] == ch {
                        let byte_pos = char_to_byte(text, i);
                        return VimAction::MoveCursor(byte_pos);
                    }
                }
            }
            FindDir::ForwardTill => {
                for i in (char_idx + 1)..chars.len() {
                    if chars[i] == ch && i > 0 {
                        let byte_pos = char_to_byte(text, i - 1);
                        return VimAction::MoveCursor(byte_pos);
                    }
                }
            }
            FindDir::BackwardTill => {
                for i in (0..char_idx).rev() {
                    if chars[i] == ch {
                        let byte_pos = char_to_byte(text, i + 1);
                        return VimAction::MoveCursor(byte_pos);
                    }
                }
            }
        }
        VimAction::None
    }

    fn do_replace(&mut self, key: char, text: &str, cursor: usize) -> VimAction {
        if cursor < text.len() {
            let end = next_char_boundary(text, cursor);
            // Delete current char and insert replacement -- caller handles sequencing.
            // We return DeleteRange; the caller can insert afterwards.
            // For simplicity, we return Paste which replaces at cursor.
            self.last_change = Some(vec!['r', key]);
            self.register = key.to_string();
            VimAction::DeleteRange(cursor, end)
        } else {
            VimAction::None
        }
    }
}

// ── Motion utilities (byte-offset aware) ────────────────────────────

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

fn move_left(text: &str, cursor: usize, count: usize) -> usize {
    let mut pos = cursor;
    for _ in 0..count {
        pos = prev_char_boundary(text, pos);
    }
    pos
}

fn move_right(text: &str, cursor: usize, count: usize) -> usize {
    let mut pos = cursor;
    for _ in 0..count {
        if pos < text.len() {
            pos = next_char_boundary(text, pos);
        }
    }
    pos.min(text.len().saturating_sub(1).max(0))
}

fn word_forward(text: &str, cursor: usize, count: usize) -> usize {
    let bytes = text.as_bytes();
    let mut pos = cursor;
    for _ in 0..count {
        // Skip current word chars.
        while pos < bytes.len() && is_word_byte(bytes[pos]) {
            pos += 1;
        }
        // Skip non-word chars (whitespace/punctuation).
        while pos < bytes.len() && !is_word_byte(bytes[pos]) {
            pos += 1;
        }
    }
    pos.min(text.len())
}

fn word_backward(text: &str, cursor: usize, count: usize) -> usize {
    let bytes = text.as_bytes();
    let mut pos = cursor;
    for _ in 0..count {
        // Move back past whitespace.
        while pos > 0 && !is_word_byte(bytes[pos.saturating_sub(1)]) {
            pos -= 1;
        }
        // Move back past word chars.
        while pos > 0 && is_word_byte(bytes[pos.saturating_sub(1)]) {
            pos -= 1;
        }
    }
    pos
}

fn word_end(text: &str, cursor: usize, count: usize) -> usize {
    let bytes = text.as_bytes();
    let mut pos = cursor;
    for _ in 0..count {
        // Advance past current position.
        if pos < bytes.len() {
            pos += 1;
        }
        // Skip whitespace.
        while pos < bytes.len() && !is_word_byte(bytes[pos]) {
            pos += 1;
        }
        // Advance to end of word.
        while pos < bytes.len().saturating_sub(1) && is_word_byte(bytes[pos + 1]) {
            pos += 1;
        }
    }
    pos.min(text.len().saturating_sub(1).max(0))
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Convert a char index to a byte offset.
fn char_to_byte(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_in_insert_mode() {
        let vim = VimState::new();
        assert_eq!(vim.mode, VimMode::Insert);
    }

    #[test]
    fn escape_switches_to_normal() {
        let mut vim = VimState::new();
        let action = vim.handle_key('\x1b', "hello", 3);
        assert_eq!(action, VimAction::SwitchToNormal);
        assert_eq!(vim.mode, VimMode::Normal);
    }

    #[test]
    fn insert_char_in_insert_mode() {
        let mut vim = VimState::new();
        let action = vim.handle_key('x', "", 0);
        assert_eq!(action, VimAction::InsertChar('x'));
    }

    #[test]
    fn normal_h_l_movement() {
        let mut vim = VimState::new();
        vim.mode = VimMode::Normal;
        vim.command = NormalCommand::Idle;

        let action = vim.handle_key('l', "hello", 0);
        assert_eq!(action, VimAction::MoveCursor(1));

        let action = vim.handle_key('h', "hello", 3);
        assert_eq!(action, VimAction::MoveCursor(2));
    }

    #[test]
    fn normal_w_b_word_motion() {
        let mut vim = VimState::new();
        vim.mode = VimMode::Normal;
        vim.command = NormalCommand::Idle;

        let text = "hello world";
        let action = vim.handle_key('w', text, 0);
        assert_eq!(action, VimAction::MoveCursor(6));

        let action = vim.handle_key('b', text, 6);
        assert_eq!(action, VimAction::MoveCursor(0));
    }

    #[test]
    fn i_enters_insert() {
        let mut vim = VimState::new();
        vim.mode = VimMode::Normal;
        vim.command = NormalCommand::Idle;
        let action = vim.handle_key('i', "test", 2);
        assert_eq!(action, VimAction::SwitchToInsert);
        assert_eq!(vim.mode, VimMode::Insert);
    }

    #[test]
    fn dd_deletes_line() {
        let mut vim = VimState::new();
        vim.mode = VimMode::Normal;
        vim.command = NormalCommand::Idle;

        // First 'd' sets operator.
        let action = vim.handle_key('d', "hello", 0);
        assert_eq!(action, VimAction::None);

        // Second 'd' deletes whole line.
        let action = vim.handle_key('d', "hello", 0);
        assert_eq!(action, VimAction::DeleteRange(0, 5));
        assert_eq!(vim.register, "hello");
    }

    #[test]
    fn x_deletes_char_under_cursor() {
        let mut vim = VimState::new();
        vim.mode = VimMode::Normal;
        vim.command = NormalCommand::Idle;

        let action = vim.handle_key('x', "abc", 1);
        assert_eq!(action, VimAction::DeleteRange(1, 2));
    }

    #[test]
    fn yy_yanks_line() {
        let mut vim = VimState::new();
        vim.mode = VimMode::Normal;
        vim.command = NormalCommand::Idle;

        vim.handle_key('y', "hello", 0);
        let action = vim.handle_key('y', "hello", 0);
        assert_eq!(action, VimAction::Yank("hello".to_string()));
        assert_eq!(vim.register, "hello");
    }

    #[test]
    fn p_pastes_register() {
        let mut vim = VimState::new();
        vim.mode = VimMode::Normal;
        vim.command = NormalCommand::Idle;
        vim.register = "world".to_string();

        let action = vim.handle_key('p', "hello ", 6);
        assert_eq!(action, VimAction::Paste("world".to_string()));
    }

    #[test]
    fn f_finds_char_forward() {
        let mut vim = VimState::new();
        vim.mode = VimMode::Normal;
        vim.command = NormalCommand::Idle;

        let action = vim.handle_key('f', "abcdef", 0);
        assert_eq!(action, VimAction::None); // waiting for char

        let action = vim.handle_key('d', "abcdef", 0);
        assert_eq!(action, VimAction::MoveCursor(3));
    }
}
