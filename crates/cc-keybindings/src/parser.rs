//! Parse and format keybinding strings.
//!
//! Handles strings like `"Ctrl+K"`, `"Shift+Enter"`, `"F1"`, `"Alt+Shift+A"`.
//! Modifier order in the formatted output is always Ctrl, Alt, Shift.

use cc_error::{CcError, CcResult};

use crate::{Key, KeyCombo, Modifiers};

/// Parse a human-readable keybinding string into a [`KeyCombo`].
///
/// Examples: `"Ctrl+C"`, `"Shift+Enter"`, `"F1"`, `"Alt+Shift+A"`, `"PageUp"`.
pub fn parse_key_combo(s: &str) -> CcResult<KeyCombo> {
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    if parts.is_empty() {
        return Err(CcError::Config("empty key combo string".into()));
    }

    let mut modifiers = Modifiers::default();
    let mut key_part: Option<&str> = None;

    for (i, part) in parts.iter().enumerate() {
        let lower = part.to_ascii_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => modifiers.ctrl = true,
            "alt" | "meta" | "option" => modifiers.alt = true,
            "shift" => modifiers.shift = true,
            _ => {
                if i != parts.len() - 1 {
                    return Err(CcError::Config(format!(
                        "unexpected modifier '{part}' (must be Ctrl, Alt, or Shift)"
                    )));
                }
                key_part = Some(part);
            }
        }
    }

    let key_str = key_part.ok_or_else(|| {
        CcError::Config("key combo has modifiers but no key".into())
    })?;

    let key = parse_key_name(key_str)?;

    Ok(KeyCombo { key, modifiers })
}

/// Format a [`KeyCombo`] back to a human-readable string.
pub fn format_key_combo(combo: &KeyCombo) -> String {
    let mut parts = Vec::new();
    if combo.modifiers.ctrl {
        parts.push("Ctrl".to_string());
    }
    if combo.modifiers.alt {
        parts.push("Alt".to_string());
    }
    if combo.modifiers.shift {
        parts.push("Shift".to_string());
    }
    parts.push(format_key_name(&combo.key));
    parts.join("+")
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn parse_key_name(s: &str) -> CcResult<Key> {
    let lower = s.to_ascii_lowercase();
    match lower.as_str() {
        "enter" | "return" => Ok(Key::Enter),
        "esc" | "escape" => Ok(Key::Escape),
        "tab" => Ok(Key::Tab),
        "backspace" | "bs" => Ok(Key::Backspace),
        "delete" | "del" => Ok(Key::Delete),
        "up" => Ok(Key::Up),
        "down" => Ok(Key::Down),
        "left" => Ok(Key::Left),
        "right" => Ok(Key::Right),
        "home" => Ok(Key::Home),
        "end" => Ok(Key::End),
        "pageup" | "pgup" => Ok(Key::PageUp),
        "pagedown" | "pgdn" => Ok(Key::PageDown),
        "insert" | "ins" => Ok(Key::Insert),
        _ if lower.starts_with('f') && lower.len() >= 2 => {
            let num: u8 = lower[1..]
                .parse()
                .map_err(|_| CcError::Config(format!("invalid function key: {s}")))?;
            if !(1..=24).contains(&num) {
                return Err(CcError::Config(format!("F key out of range: {num}")));
            }
            Ok(Key::F(num))
        }
        _ => {
            let chars: Vec<char> = s.chars().collect();
            if chars.len() == 1 {
                Ok(Key::Char(chars[0]))
            } else {
                Err(CcError::Config(format!("unknown key name: {s}")))
            }
        }
    }
}

fn format_key_name(key: &Key) -> String {
    match key {
        Key::Char(c) => c.to_uppercase().to_string(),
        Key::F(n) => format!("F{n}"),
        Key::Enter => "Enter".into(),
        Key::Escape => "Escape".into(),
        Key::Tab => "Tab".into(),
        Key::Backspace => "Backspace".into(),
        Key::Delete => "Delete".into(),
        Key::Up => "Up".into(),
        Key::Down => "Down".into(),
        Key::Left => "Left".into(),
        Key::Right => "Right".into(),
        Key::Home => "Home".into(),
        Key::End => "End".into(),
        Key::PageUp => "PageUp".into(),
        Key::PageDown => "PageDown".into(),
        Key::Insert => "Insert".into(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_char() {
        let kc = parse_key_combo("A").unwrap();
        assert_eq!(kc.key, Key::Char('A'));
        assert_eq!(kc.modifiers, Modifiers::none());
    }

    #[test]
    fn test_parse_ctrl_c() {
        let kc = parse_key_combo("Ctrl+C").unwrap();
        assert_eq!(kc.key, Key::Char('C'));
        assert!(kc.modifiers.ctrl);
        assert!(!kc.modifiers.alt);
    }

    #[test]
    fn test_parse_alt_shift() {
        let kc = parse_key_combo("Alt+Shift+X").unwrap();
        assert!(kc.modifiers.alt);
        assert!(kc.modifiers.shift);
        assert!(!kc.modifiers.ctrl);
        assert_eq!(kc.key, Key::Char('X'));
    }

    #[test]
    fn test_parse_f_key() {
        let kc = parse_key_combo("F1").unwrap();
        assert_eq!(kc.key, Key::F(1));

        let kc12 = parse_key_combo("F12").unwrap();
        assert_eq!(kc12.key, Key::F(12));
    }

    #[test]
    fn test_parse_special_keys() {
        assert_eq!(parse_key_combo("Enter").unwrap().key, Key::Enter);
        assert_eq!(parse_key_combo("Escape").unwrap().key, Key::Escape);
        assert_eq!(parse_key_combo("Tab").unwrap().key, Key::Tab);
        assert_eq!(parse_key_combo("PageUp").unwrap().key, Key::PageUp);
    }

    #[test]
    fn test_roundtrip() {
        let combos = vec![
            "Ctrl+C",
            "Alt+Shift+A",
            "F1",
            "Enter",
            "Shift+Enter",
            "Ctrl+Alt+Delete",
        ];
        for s in combos {
            let kc = parse_key_combo(s).unwrap();
            let formatted = format_key_combo(&kc);
            let reparsed = parse_key_combo(&formatted).unwrap();
            assert_eq!(kc, reparsed, "roundtrip failed for {s}");
        }
    }

    #[test]
    fn test_parse_error_on_empty() {
        assert!(parse_key_combo("").is_err());
    }

    #[test]
    fn test_parse_error_on_unknown_key() {
        assert!(parse_key_combo("Ctrl+FooBar").is_err());
    }
}
