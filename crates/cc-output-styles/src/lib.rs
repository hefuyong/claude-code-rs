//! Output style system for Claude Code RS.
//!
//! Styles control how Claude formats its responses.  Each style is a
//! Markdown file with YAML front-matter that specifies metadata plus a
//! body of prose instructions that are injected into the system prompt.
//!
//! Styles are loaded from:
//!   - Built-in defaults (concise, verbose, technical, friendly)
//!   - User-level: `~/.claude/output-styles/*.md`
//!   - Project-level: `<project>/.claude/output-styles/*.md`

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cc_error::{CcError, CcResult};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// StyleSource
// ---------------------------------------------------------------------------

/// Where a style was loaded from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum StyleSource {
    /// Compiled into the binary.
    Builtin,
    /// Found under `<project>/.claude/output-styles/`.
    Project(PathBuf),
    /// Found under `~/.claude/output-styles/`.
    User(PathBuf),
}

// ---------------------------------------------------------------------------
// OutputStyle
// ---------------------------------------------------------------------------

/// A single output style definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputStyle {
    /// Short machine-friendly name (e.g. `"concise"`).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// If `true`, the default coding-specific instructions are kept in the
    /// system prompt alongside the style instructions.
    pub keep_coding_instructions: bool,
    /// When set, this style is forcefully activated when a certain plugin
    /// is loaded.
    pub force_for_plugin: Option<String>,
    /// The Markdown body that is injected into the system prompt.
    pub content: String,
    /// Where the style was loaded from.
    pub source: StyleSource,
}

// ---------------------------------------------------------------------------
// Front-matter (YAML)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct StyleFrontMatter {
    name: Option<String>,
    description: Option<String>,
    #[serde(rename = "keep-coding-instructions", default)]
    keep_coding_instructions: bool,
    #[serde(rename = "force-for-plugin")]
    force_for_plugin: Option<String>,
}

// ---------------------------------------------------------------------------
// OutputStyleRegistry
// ---------------------------------------------------------------------------

/// Registry that holds all known output styles and tracks which one is
/// currently active.
pub struct OutputStyleRegistry {
    styles: HashMap<String, OutputStyle>,
    active: Option<String>,
}

impl OutputStyleRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            styles: HashMap::new(),
            active: None,
        }
    }

    /// Populate the registry with built-in styles.
    pub fn register_builtins(&mut self) {
        for style in builtin_styles() {
            self.styles.insert(style.name.clone(), style);
        }
    }

    /// Load additional styles from disk.
    ///
    /// Scans `<project_root>/.claude/output-styles/*.md` and
    /// `~/.claude/output-styles/*.md`.  Project styles take precedence
    /// over user styles with the same name.
    pub async fn load_from_disk(&mut self, project_root: &Path) -> CcResult<()> {
        // User-level styles
        if let Some(home) = dirs::home_dir() {
            let user_dir = home.join(".claude").join("output-styles");
            self.load_styles_from_dir(&user_dir, |p| StyleSource::User(p))?;
        }

        // Project-level styles (override user-level)
        let project_dir = project_root.join(".claude").join("output-styles");
        self.load_styles_from_dir(&project_dir, |p| StyleSource::Project(p))?;

        Ok(())
    }

    /// Set the active style by name.
    pub fn set_active(&mut self, name: &str) -> CcResult<()> {
        if !self.styles.contains_key(name) {
            return Err(CcError::NotFound(format!("output style '{}' not found", name)));
        }
        self.active = Some(name.to_string());
        Ok(())
    }

    /// Get the currently active style, if any.
    pub fn active_style(&self) -> Option<&OutputStyle> {
        self.active.as_ref().and_then(|n| self.styles.get(n))
    }

    /// List all registered styles.
    pub fn list(&self) -> Vec<&OutputStyle> {
        self.styles.values().collect()
    }

    /// Look up a style by name.
    pub fn get(&self, name: &str) -> Option<&OutputStyle> {
        self.styles.get(name)
    }

    // -- internal helpers ---------------------------------------------------

    fn load_styles_from_dir<F>(&mut self, dir: &Path, source_fn: F) -> CcResult<()>
    where
        F: Fn(PathBuf) -> StyleSource,
    {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()), // directory doesn't exist -- not an error
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let content = std::fs::read_to_string(&path).map_err(CcError::Io)?;
            let source = source_fn(path.clone());
            match parse_style_file(&content, source) {
                Ok(style) => {
                    tracing::debug!(name = %style.name, path = ?path, "loaded output style");
                    self.styles.insert(style.name.clone(), style);
                }
                Err(e) => {
                    tracing::warn!(path = ?path, error = %e, "skipping invalid style file");
                }
            }
        }

        Ok(())
    }
}

impl Default for OutputStyleRegistry {
    fn default() -> Self {
        let mut r = Self::new();
        r.register_builtins();
        r
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse a Markdown file with optional YAML front-matter delimited by `---`.
///
/// ```text
/// ---
/// name: concise
/// description: Short, to-the-point answers
/// keep-coding-instructions: true
/// ---
///
/// Be concise. Omit filler words ...
/// ```
fn parse_style_file(content: &str, source: StyleSource) -> CcResult<OutputStyle> {
    let (front_matter, body) = split_front_matter(content);

    let fm: StyleFrontMatter = if let Some(yaml) = front_matter {
        serde_yaml::from_str(&yaml)
            .map_err(|e| CcError::Serialization(format!("invalid YAML front-matter: {}", e)))?
    } else {
        StyleFrontMatter {
            name: None,
            description: None,
            keep_coding_instructions: true,
            force_for_plugin: None,
        }
    };

    let name = fm.name.ok_or_else(|| {
        CcError::Config("style file missing 'name' in front-matter".into())
    })?;

    Ok(OutputStyle {
        name,
        description: fm.description.unwrap_or_default(),
        keep_coding_instructions: fm.keep_coding_instructions,
        force_for_plugin: fm.force_for_plugin,
        content: body.trim().to_string(),
        source,
    })
}

/// Split a document into optional YAML front-matter and the remaining body.
fn split_front_matter(content: &str) -> (Option<String>, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content.to_string());
    }

    // Find the closing `---`.
    let after_first = &trimmed[3..];
    if let Some(end) = after_first.find("\n---") {
        let yaml = after_first[..end].to_string();
        let body = after_first[end + 4..].to_string(); // skip "\n---"
        (Some(yaml), body)
    } else {
        (None, content.to_string())
    }
}

// ---------------------------------------------------------------------------
// Built-in styles
// ---------------------------------------------------------------------------

fn builtin_styles() -> Vec<OutputStyle> {
    vec![
        OutputStyle {
            name: "concise".into(),
            description: "Short, direct responses. Minimal filler.".into(),
            keep_coding_instructions: true,
            force_for_plugin: None,
            content: "Be concise. Give short, direct answers. \
                      Omit filler words and unnecessary explanations. \
                      Use bullet points when listing multiple items. \
                      Prefer code over prose when answering coding questions."
                .into(),
            source: StyleSource::Builtin,
        },
        OutputStyle {
            name: "verbose".into(),
            description: "Detailed, thorough explanations.".into(),
            keep_coding_instructions: true,
            force_for_plugin: None,
            content: "Provide detailed, thorough explanations. \
                      Include context, rationale, and alternatives. \
                      Use examples when helpful. \
                      Structure long answers with headers and sections."
                .into(),
            source: StyleSource::Builtin,
        },
        OutputStyle {
            name: "technical".into(),
            description: "Precise, jargon-appropriate for engineers.".into(),
            keep_coding_instructions: true,
            force_for_plugin: None,
            content: "Write for a senior software engineer audience. \
                      Use precise technical terminology. \
                      Include complexity analysis where relevant. \
                      Reference language specifications or RFCs when applicable. \
                      Prefer accuracy over simplicity."
                .into(),
            source: StyleSource::Builtin,
        },
        OutputStyle {
            name: "friendly".into(),
            description: "Warm, approachable tone.".into(),
            keep_coding_instructions: true,
            force_for_plugin: None,
            content: "Use a warm, approachable tone. \
                      Explain concepts step by step. \
                      Encourage the reader. \
                      Avoid intimidating jargon; \
                      define technical terms when you first use them."
                .into(),
            source: StyleSource::Builtin,
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
    fn test_builtin_styles_registered() {
        let reg = OutputStyleRegistry::default();
        assert!(reg.get("concise").is_some());
        assert!(reg.get("verbose").is_some());
        assert!(reg.get("technical").is_some());
        assert!(reg.get("friendly").is_some());
    }

    #[test]
    fn test_set_active_style() {
        let mut reg = OutputStyleRegistry::default();
        assert!(reg.active_style().is_none());

        reg.set_active("verbose").unwrap();
        let active = reg.active_style().unwrap();
        assert_eq!(active.name, "verbose");
    }

    #[test]
    fn test_set_active_unknown_errors() {
        let mut reg = OutputStyleRegistry::default();
        let err = reg.set_active("nonexistent");
        assert!(err.is_err());
    }

    #[test]
    fn test_parse_style_file_with_frontmatter() {
        let content = "\
---
name: custom
description: My custom style
keep-coding-instructions: false
---

Write everything in haiku form.
";
        let style = parse_style_file(content, StyleSource::Builtin).unwrap();
        assert_eq!(style.name, "custom");
        assert_eq!(style.description, "My custom style");
        assert!(!style.keep_coding_instructions);
        assert!(style.content.contains("haiku"));
    }

    #[test]
    fn test_parse_style_file_missing_name() {
        let content = "\
---
description: oops no name
---

some content
";
        let result = parse_style_file(content, StyleSource::Builtin);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_style_no_frontmatter() {
        let content = "Just plain text, no YAML.";
        let result = parse_style_file(content, StyleSource::Builtin);
        // Should fail because 'name' is required.
        assert!(result.is_err());
    }

    #[test]
    fn test_list_returns_all() {
        let reg = OutputStyleRegistry::default();
        let list = reg.list();
        assert_eq!(list.len(), 4);
    }

    #[test]
    fn test_split_front_matter() {
        let input = "---\nfoo: bar\n---\nbody text";
        let (fm, body) = split_front_matter(input);
        assert!(fm.is_some());
        assert!(fm.unwrap().contains("foo: bar"));
        assert!(body.contains("body text"));
    }

    #[test]
    fn test_split_front_matter_none() {
        let input = "no front matter here";
        let (fm, body) = split_front_matter(input);
        assert!(fm.is_none());
        assert_eq!(body, input);
    }
}
