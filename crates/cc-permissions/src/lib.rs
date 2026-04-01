//! Permission system for Claude Code RS.
//!
//! Controls which tools can be executed and which paths can be accessed,
//! matching the permission model from the original Claude Code TypeScript.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The permission mode governing how tool calls are authorized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionMode {
    /// Default interactive mode: asks user for unrecognized tools.
    Default,
    /// Plan mode: only read-only tools are allowed without asking.
    Plan,
    /// Auto mode: allow all tools that are not explicitly denied.
    Auto,
    /// Bypass mode: skip all permission checks (for testing / trusted envs).
    Bypass,
}

impl Default for PermissionMode {
    fn default() -> Self {
        Self::Default
    }
}

/// The behavior a permission rule prescribes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionBehavior {
    /// Always allow without prompting.
    Allow,
    /// Always deny.
    Deny,
    /// Ask the user for confirmation.
    Ask,
}

/// A single permission rule that matches a tool by name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// The tool name this rule applies to. Supports glob patterns (e.g. "file_*").
    pub tool_name: String,
    /// What to do when the rule matches.
    pub behavior: PermissionBehavior,
    /// Where this rule came from: "user", "project", or "cli".
    pub source: String,
}

impl PermissionRule {
    /// Check whether this rule matches the given tool name.
    pub fn matches(&self, tool_name: &str) -> bool {
        if self.tool_name == "*" {
            return true;
        }
        if self.tool_name.contains('*') {
            let pattern = self.tool_name.replace('*', "");
            if self.tool_name.starts_with('*') && self.tool_name.ends_with('*') {
                tool_name.contains(&pattern)
            } else if self.tool_name.starts_with('*') {
                tool_name.ends_with(&pattern)
            } else if self.tool_name.ends_with('*') {
                tool_name.starts_with(&pattern)
            } else {
                tool_name == self.tool_name
            }
        } else {
            tool_name == self.tool_name
        }
    }
}

/// The result of checking permissions for a tool call.
#[derive(Debug, Clone)]
pub enum PermissionCheckResult {
    /// The call is allowed.
    Allow,
    /// The call is denied.
    Deny { reason: String },
    /// The user should be asked for confirmation.
    Ask { message: String },
}

/// Holds the full permission context for a session.
#[derive(Debug, Clone)]
pub struct PermissionContext {
    /// The current permission mode.
    pub mode: PermissionMode,
    /// Rules that always allow certain tools.
    pub always_allow_rules: Vec<PermissionRule>,
    /// Rules that always deny certain tools.
    pub always_deny_rules: Vec<PermissionRule>,
    /// Directories the tools are allowed to operate within.
    pub working_directories: Vec<PathBuf>,
}

impl PermissionContext {
    /// Create a new permission context.
    pub fn new(mode: PermissionMode, working_dirs: Vec<PathBuf>) -> Self {
        Self {
            mode,
            always_allow_rules: Vec::new(),
            always_deny_rules: Vec::new(),
            working_directories: working_dirs,
        }
    }

    /// Add a permission rule. Allow rules go to `always_allow_rules`,
    /// Deny rules go to `always_deny_rules`, Ask rules are ignored (that's the default).
    pub fn add_rule(&mut self, rule: PermissionRule) {
        match rule.behavior {
            PermissionBehavior::Allow => self.always_allow_rules.push(rule),
            PermissionBehavior::Deny => self.always_deny_rules.push(rule),
            PermissionBehavior::Ask => {
                // Ask is the default; no need to store.
            }
        }
    }

    /// Check whether a tool call is permitted.
    pub fn check(&self, tool_name: &str, _input: &serde_json::Value) -> PermissionCheckResult {
        // Bypass mode allows everything.
        if self.mode == PermissionMode::Bypass {
            return PermissionCheckResult::Allow;
        }

        // Deny rules take precedence.
        for rule in &self.always_deny_rules {
            if rule.matches(tool_name) {
                return PermissionCheckResult::Deny {
                    reason: format!(
                        "Tool '{}' is denied by {} rule (pattern: '{}')",
                        tool_name, rule.source, rule.tool_name
                    ),
                };
            }
        }

        // Allow rules come next.
        for rule in &self.always_allow_rules {
            if rule.matches(tool_name) {
                return PermissionCheckResult::Allow;
            }
        }

        // Auto mode allows everything not explicitly denied.
        if self.mode == PermissionMode::Auto {
            return PermissionCheckResult::Allow;
        }

        // Default mode: ask the user.
        PermissionCheckResult::Ask {
            message: format!("Allow tool '{}'?", tool_name),
        }
    }

    /// Check whether a filesystem path is within the allowed working directories.
    ///
    /// For write operations we are stricter: the path must be *inside*
    /// a working directory (not merely equal to it).
    pub fn is_path_allowed(&self, path: &Path, write: bool) -> bool {
        if self.mode == PermissionMode::Bypass {
            return true;
        }

        // Canonicalize once (best-effort; fall back to the raw path).
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

        for wd in &self.working_directories {
            let canon_wd = std::fs::canonicalize(wd).unwrap_or_else(|_| wd.clone());
            if write {
                // For writes the path must be strictly inside the working dir.
                if canonical.starts_with(&canon_wd) && canonical != canon_wd {
                    return true;
                }
            } else {
                // Reads are allowed if the path equals or is inside the working dir.
                if canonical.starts_with(&canon_wd) {
                    return true;
                }
            }
        }

        false
    }
}

/// Tracks consecutive denials so the UI can offer to switch modes.
#[derive(Debug, Clone)]
pub struct DenialTracker {
    /// How many consecutive denials have occurred.
    pub denial_count: u32,
    /// After this many denials we surface a suggestion.
    pub threshold: u32,
}

impl Default for DenialTracker {
    fn default() -> Self {
        Self {
            denial_count: 0,
            threshold: 3,
        }
    }
}

impl DenialTracker {
    /// Record a denial. Returns `true` when the threshold has been reached.
    pub fn record_denial(&mut self) -> bool {
        self.denial_count += 1;
        self.denial_count >= self.threshold
    }

    /// Reset the counter (e.g. after the user allows something).
    pub fn reset(&mut self) {
        self.denial_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypass_mode_allows_everything() {
        let ctx = PermissionContext::new(PermissionMode::Bypass, vec![]);
        match ctx.check("anything", &serde_json::Value::Null) {
            PermissionCheckResult::Allow => {}
            other => panic!("expected Allow, got {:?}", other),
        }
    }

    #[test]
    fn deny_rule_takes_precedence() {
        let mut ctx = PermissionContext::new(PermissionMode::Auto, vec![]);
        ctx.add_rule(PermissionRule {
            tool_name: "bash".into(),
            behavior: PermissionBehavior::Deny,
            source: "project".into(),
        });
        match ctx.check("bash", &serde_json::Value::Null) {
            PermissionCheckResult::Deny { .. } => {}
            other => panic!("expected Deny, got {:?}", other),
        }
    }

    #[test]
    fn auto_mode_allows_unmatched() {
        let ctx = PermissionContext::new(PermissionMode::Auto, vec![]);
        match ctx.check("file_read", &serde_json::Value::Null) {
            PermissionCheckResult::Allow => {}
            other => panic!("expected Allow, got {:?}", other),
        }
    }

    #[test]
    fn default_mode_asks_for_unmatched() {
        let ctx = PermissionContext::new(PermissionMode::Default, vec![]);
        match ctx.check("file_read", &serde_json::Value::Null) {
            PermissionCheckResult::Ask { .. } => {}
            other => panic!("expected Ask, got {:?}", other),
        }
    }

    #[test]
    fn wildcard_rule_matches() {
        let rule = PermissionRule {
            tool_name: "file_*".into(),
            behavior: PermissionBehavior::Allow,
            source: "user".into(),
        };
        assert!(rule.matches("file_read"));
        assert!(rule.matches("file_write"));
        assert!(!rule.matches("bash"));
    }

    #[test]
    fn denial_tracker_threshold() {
        let mut tracker = DenialTracker::default();
        assert!(!tracker.record_denial());
        assert!(!tracker.record_denial());
        assert!(tracker.record_denial()); // 3rd time
        tracker.reset();
        assert_eq!(tracker.denial_count, 0);
    }
}
