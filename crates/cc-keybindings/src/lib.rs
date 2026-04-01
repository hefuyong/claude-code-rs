//! Customizable keybinding system for Claude Code RS.
//!
//! Provides a registry that maps [`KeyCombo`]s (key + modifiers) to
//! named actions, filtered by [`KeyContext`].  Users can override the
//! defaults by placing a JSON file at `~/.config/claude-code/keybindings.json`.
//!
//! # Modules
//!
//! * [`parser`] -- parse / format `"Ctrl+K"` style strings.
//! * [`resolver`] -- bridge from [`crossterm::event::KeyEvent`] to [`KeyCombo`].

pub mod parser;
pub mod resolver;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use cc_error::{CcError, CcResult};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

// Re-exports for convenience.
pub use parser::{format_key_combo, parse_key_combo};

// ---------------------------------------------------------------------------
// Key types
// ---------------------------------------------------------------------------

/// A physical key on the keyboard.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum Key {
    Char(char),
    F(u8),
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
}

/// Modifier flags (Ctrl, Alt, Shift).
#[derive(Debug, Clone, Default, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Modifiers {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn ctrl() -> Self {
        Self { ctrl: true, ..Default::default() }
    }

    pub fn alt() -> Self {
        Self { alt: true, ..Default::default() }
    }

    pub fn shift() -> Self {
        Self { shift: true, ..Default::default() }
    }

    pub fn ctrl_shift() -> Self {
        Self { ctrl: true, shift: true, ..Default::default() }
    }
}

/// A combination of a [`Key`] and zero or more [`Modifiers`].
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct KeyCombo {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl KeyCombo {
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    /// Shorthand: plain key, no modifiers.
    pub fn plain(key: Key) -> Self {
        Self { key, modifiers: Modifiers::none() }
    }

    /// Shorthand: Ctrl + key.
    pub fn ctrl(key: Key) -> Self {
        Self { key, modifiers: Modifiers::ctrl() }
    }

    /// Shorthand: Shift + key.
    pub fn shift(key: Key) -> Self {
        Self { key, modifiers: Modifiers::shift() }
    }
}

// ---------------------------------------------------------------------------
// Context & binding
// ---------------------------------------------------------------------------

/// The context in which a keybinding is active.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyContext {
    /// Active everywhere.
    Global,
    /// Active when the text input is focused.
    Input,
    /// Active in normal (non-editing) mode.
    Normal,
    /// Active while the search bar is open.
    Search,
    /// Active during a permission prompt.
    PermissionPrompt,
}

/// A single keybinding entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keybinding {
    pub combo: KeyCombo,
    pub action: String,
    pub description: String,
    pub context: KeyContext,
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Manages all keybindings, including defaults and user overrides.
pub struct KeybindingRegistry {
    /// Active bindings keyed by combo.
    bindings: HashMap<KeyCombo, Keybinding>,
    /// Combos that cannot be overridden by the user.
    reserved: HashSet<KeyCombo>,
}

impl KeybindingRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
            reserved: HashSet::new(),
        }
    }

    /// Populate the registry with the built-in default keybindings.
    pub fn register_defaults(&mut self) {
        for kb in default_bindings() {
            let combo = kb.combo.clone();
            self.bindings.insert(combo.clone(), kb);
            // Mark Ctrl+C and Ctrl+D as reserved -- they must not be
            // overridden because they control process lifecycle.
            if combo == KeyCombo::ctrl(Key::Char('c'))
                || combo == KeyCombo::ctrl(Key::Char('d'))
            {
                self.reserved.insert(combo);
            }
        }
    }

    /// Load user-defined keybinding overrides from a JSON file.
    ///
    /// The file is expected at `<config_dir>/keybindings.json` and
    /// contains an array of keybinding objects.
    pub fn load_user_bindings(&mut self, config_dir: &Path) -> CcResult<()> {
        let path = config_dir.join("keybindings.json");
        if !path.exists() {
            debug!("no user keybindings file at {}", path.display());
            return Ok(());
        }

        let data = std::fs::read_to_string(&path)?;
        let overrides: Vec<Keybinding> = serde_json::from_str(&data)
            .map_err(|e| CcError::Config(format!("bad keybindings.json: {e}")))?;

        for kb in overrides {
            if self.reserved.contains(&kb.combo) {
                warn!(
                    action = %kb.action,
                    "ignoring override for reserved keybinding {}",
                    format_key_combo(&kb.combo)
                );
                continue;
            }
            self.bindings.insert(kb.combo.clone(), kb);
        }

        debug!(path = %path.display(), "loaded user keybindings");
        Ok(())
    }

    /// Add or replace a binding.
    pub fn bind(
        &mut self,
        combo: KeyCombo,
        action: &str,
        desc: &str,
        ctx: KeyContext,
    ) -> CcResult<()> {
        if self.reserved.contains(&combo) {
            return Err(CcError::PermissionDenied(format!(
                "{} is reserved",
                format_key_combo(&combo)
            )));
        }
        self.bindings.insert(
            combo.clone(),
            Keybinding {
                combo,
                action: action.to_string(),
                description: desc.to_string(),
                context: ctx,
            },
        );
        Ok(())
    }

    /// Remove a binding.
    pub fn unbind(&mut self, combo: &KeyCombo) -> CcResult<()> {
        if self.reserved.contains(combo) {
            return Err(CcError::PermissionDenied(format!(
                "{} is reserved",
                format_key_combo(combo)
            )));
        }
        self.bindings.remove(combo);
        Ok(())
    }

    /// Resolve a key combo in the given context.
    ///
    /// Returns the matching [`Keybinding`] if the combo is bound and
    /// the context matches (or the binding is [`KeyContext::Global`]).
    pub fn resolve(&self, combo: &KeyCombo, context: &KeyContext) -> Option<&Keybinding> {
        let kb = self.bindings.get(combo)?;
        if kb.context == *context || kb.context == KeyContext::Global {
            Some(kb)
        } else {
            None
        }
    }

    /// List all bindings.
    pub fn list(&self) -> Vec<&Keybinding> {
        self.bindings.values().collect()
    }

    /// List bindings that apply in a specific context (includes global).
    pub fn list_for_context(&self, ctx: &KeyContext) -> Vec<&Keybinding> {
        self.bindings
            .values()
            .filter(|kb| kb.context == *ctx || kb.context == KeyContext::Global)
            .collect()
    }

    /// Check whether a combo is reserved.
    pub fn is_reserved(&self, combo: &KeyCombo) -> bool {
        self.reserved.contains(combo)
    }
}

impl Default for KeybindingRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Default bindings
// ---------------------------------------------------------------------------

fn default_bindings() -> Vec<Keybinding> {
    vec![
        Keybinding {
            combo: KeyCombo::ctrl(Key::Char('c')),
            action: "exit".into(),
            description: "Exit the application".into(),
            context: KeyContext::Global,
        },
        Keybinding {
            combo: KeyCombo::ctrl(Key::Char('d')),
            action: "eof".into(),
            description: "Send end-of-file".into(),
            context: KeyContext::Global,
        },
        Keybinding {
            combo: KeyCombo::plain(Key::Enter),
            action: "submit".into(),
            description: "Submit the current input".into(),
            context: KeyContext::Input,
        },
        Keybinding {
            combo: KeyCombo::shift(Key::Enter),
            action: "newline".into(),
            description: "Insert a newline without submitting".into(),
            context: KeyContext::Input,
        },
        Keybinding {
            combo: KeyCombo::plain(Key::Tab),
            action: "complete".into(),
            description: "Trigger tab completion".into(),
            context: KeyContext::Input,
        },
        Keybinding {
            combo: KeyCombo::ctrl(Key::Char('f')),
            action: "search".into(),
            description: "Open search".into(),
            context: KeyContext::Global,
        },
        Keybinding {
            combo: KeyCombo::plain(Key::F(1)),
            action: "help".into(),
            description: "Show help".into(),
            context: KeyContext::Global,
        },
        Keybinding {
            combo: KeyCombo::plain(Key::Escape),
            action: "cancel".into(),
            description: "Cancel the current operation".into(),
            context: KeyContext::Global,
        },
        Keybinding {
            combo: KeyCombo::ctrl(Key::Char('l')),
            action: "clear".into(),
            description: "Clear the screen".into(),
            context: KeyContext::Global,
        },
        Keybinding {
            combo: KeyCombo::plain(Key::PageUp),
            action: "scroll_up".into(),
            description: "Scroll up one page".into(),
            context: KeyContext::Normal,
        },
        Keybinding {
            combo: KeyCombo::plain(Key::PageDown),
            action: "scroll_down".into(),
            description: "Scroll down one page".into(),
            context: KeyContext::Normal,
        },
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_loaded() {
        let mut reg = KeybindingRegistry::new();
        reg.register_defaults();
        // We defined 11 default bindings.
        assert!(reg.list().len() >= 10);
    }

    #[test]
    fn test_ctrl_c_is_reserved() {
        let mut reg = KeybindingRegistry::new();
        reg.register_defaults();
        assert!(reg.is_reserved(&KeyCombo::ctrl(Key::Char('c'))));
        assert!(reg.is_reserved(&KeyCombo::ctrl(Key::Char('d'))));
        assert!(!reg.is_reserved(&KeyCombo::plain(Key::Tab)));
    }

    #[test]
    fn test_bind_and_resolve() {
        let mut reg = KeybindingRegistry::new();
        reg.bind(
            KeyCombo::ctrl(Key::Char('k')),
            "kill_line",
            "Kill the current line",
            KeyContext::Input,
        )
        .unwrap();

        let kb = reg
            .resolve(&KeyCombo::ctrl(Key::Char('k')), &KeyContext::Input)
            .expect("should resolve");
        assert_eq!(kb.action, "kill_line");
    }

    #[test]
    fn test_global_resolves_in_any_context() {
        let mut reg = KeybindingRegistry::new();
        reg.register_defaults();
        // Escape is Global -- should resolve even in Input context.
        assert!(reg
            .resolve(&KeyCombo::plain(Key::Escape), &KeyContext::Input)
            .is_some());
    }

    #[test]
    fn test_context_mismatch_returns_none() {
        let mut reg = KeybindingRegistry::new();
        reg.register_defaults();
        // PageUp is Normal -- should not resolve in Search context.
        assert!(reg
            .resolve(&KeyCombo::plain(Key::PageUp), &KeyContext::Search)
            .is_none());
    }

    #[test]
    fn test_cannot_bind_reserved() {
        let mut reg = KeybindingRegistry::new();
        reg.register_defaults();
        let result = reg.bind(
            KeyCombo::ctrl(Key::Char('c')),
            "noop",
            "try override",
            KeyContext::Global,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_unbind_reserved() {
        let mut reg = KeybindingRegistry::new();
        reg.register_defaults();
        let result = reg.unbind(&KeyCombo::ctrl(Key::Char('c')));
        assert!(result.is_err());
    }

    #[test]
    fn test_unbind_normal_key() {
        let mut reg = KeybindingRegistry::new();
        reg.bind(
            KeyCombo::ctrl(Key::Char('k')),
            "kill",
            "kill",
            KeyContext::Input,
        )
        .unwrap();
        assert!(reg
            .resolve(&KeyCombo::ctrl(Key::Char('k')), &KeyContext::Input)
            .is_some());

        reg.unbind(&KeyCombo::ctrl(Key::Char('k'))).unwrap();
        assert!(reg
            .resolve(&KeyCombo::ctrl(Key::Char('k')), &KeyContext::Input)
            .is_none());
    }

    #[test]
    fn test_list_for_context() {
        let mut reg = KeybindingRegistry::new();
        reg.register_defaults();
        let input_bindings = reg.list_for_context(&KeyContext::Input);
        // Should include Input-specific and Global bindings.
        assert!(input_bindings.len() >= 3);
    }
}
