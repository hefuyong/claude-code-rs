//! Bridge from [`crossterm::event::KeyEvent`] to [`KeyCombo`] and
//! configuration validation helpers.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{Key, KeyCombo, Modifiers};

/// Convert a [`crossterm::event::KeyEvent`] into our [`KeyCombo`].
pub fn from_crossterm(event: &KeyEvent) -> KeyCombo {
    let modifiers = Modifiers {
        ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
        alt: event.modifiers.contains(KeyModifiers::ALT),
        shift: event.modifiers.contains(KeyModifiers::SHIFT),
    };

    let key = match event.code {
        KeyCode::Char(c) => Key::Char(c),
        KeyCode::F(n) => Key::F(n),
        KeyCode::Enter => Key::Enter,
        KeyCode::Esc => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Delete => Key::Delete,
        KeyCode::Up => Key::Up,
        KeyCode::Down => Key::Down,
        KeyCode::Left => Key::Left,
        KeyCode::Right => Key::Right,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Insert => Key::Insert,
        // For any key codes we don't explicitly handle, fall back to
        // a null character.  This keeps the mapping total.
        _ => Key::Char('\0'),
    };

    KeyCombo { key, modifiers }
}

/// Validate that a keybindings JSON configuration value is well-formed.
///
/// Returns a list of human-readable error strings (empty = valid).
pub fn validate_config(config: &serde_json::Value) -> Vec<String> {
    let mut errors = Vec::new();

    let arr = match config.as_array() {
        Some(a) => a,
        None => {
            errors.push("keybindings config must be a JSON array".into());
            return errors;
        }
    };

    for (i, entry) in arr.iter().enumerate() {
        if !entry.is_object() {
            errors.push(format!("entry {i}: expected an object"));
            continue;
        }

        // combo (required -- must contain key)
        if entry.get("combo").is_none() {
            errors.push(format!("entry {i}: missing 'combo' field"));
        } else if entry["combo"].get("key").is_none() {
            errors.push(format!("entry {i}: combo missing 'key' field"));
        }

        // action (required)
        match entry.get("action") {
            Some(v) if v.is_string() => {}
            Some(_) => errors.push(format!("entry {i}: 'action' must be a string")),
            None => errors.push(format!("entry {i}: missing 'action' field")),
        }

        // description (required)
        match entry.get("description") {
            Some(v) if v.is_string() => {}
            Some(_) => errors.push(format!("entry {i}: 'description' must be a string")),
            None => errors.push(format!("entry {i}: missing 'description' field")),
        }

        // context (required)
        match entry.get("context") {
            Some(v) if v.is_string() => {
                let ctx = v.as_str().unwrap();
                if !["Global", "Input", "Normal", "Search", "PermissionPrompt"]
                    .contains(&ctx)
                {
                    errors.push(format!("entry {i}: unknown context '{ctx}'"));
                }
            }
            Some(_) => errors.push(format!("entry {i}: 'context' must be a string")),
            None => errors.push(format!("entry {i}: missing 'context' field")),
        }
    }

    errors
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_crossterm_char() {
        let event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        let combo = from_crossterm(&event);
        assert_eq!(combo.key, Key::Char('a'));
        assert_eq!(combo.modifiers, Modifiers::none());
    }

    #[test]
    fn test_from_crossterm_ctrl_c() {
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let combo = from_crossterm(&event);
        assert_eq!(combo.key, Key::Char('c'));
        assert!(combo.modifiers.ctrl);
    }

    #[test]
    fn test_from_crossterm_f_key() {
        let event = KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE);
        let combo = from_crossterm(&event);
        assert_eq!(combo.key, Key::F(5));
    }

    #[test]
    fn test_from_crossterm_special_keys() {
        let cases = vec![
            (KeyCode::Enter, Key::Enter),
            (KeyCode::Esc, Key::Escape),
            (KeyCode::Tab, Key::Tab),
            (KeyCode::Backspace, Key::Backspace),
            (KeyCode::PageUp, Key::PageUp),
        ];
        for (code, expected) in cases {
            let event = KeyEvent::new(code, KeyModifiers::NONE);
            assert_eq!(from_crossterm(&event).key, expected);
        }
    }

    #[test]
    fn test_validate_config_valid() {
        let config = serde_json::json!([
            {
                "combo": { "key": { "Char": "k" }, "modifiers": { "ctrl": true, "alt": false, "shift": false } },
                "action": "kill_line",
                "description": "Kill the current line",
                "context": "Input"
            }
        ]);
        let errors = validate_config(&config);
        assert!(errors.is_empty(), "expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_validate_config_missing_fields() {
        let config = serde_json::json!([
            { "combo": { "key": "Enter" } }
        ]);
        let errors = validate_config(&config);
        assert!(errors.len() >= 2, "expected at least 2 errors: {:?}", errors);
    }

    #[test]
    fn test_validate_config_not_array() {
        let config = serde_json::json!({ "foo": "bar" });
        let errors = validate_config(&config);
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("array"));
    }

    #[test]
    fn test_validate_config_bad_context() {
        let config = serde_json::json!([
            {
                "combo": { "key": "Enter" },
                "action": "submit",
                "description": "submit",
                "context": "Bogus"
            }
        ]);
        let errors = validate_config(&config);
        assert!(errors.iter().any(|e| e.contains("unknown context")));
    }
}
