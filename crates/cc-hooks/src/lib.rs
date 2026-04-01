//! Hook system for Claude Code RS.
//!
//! Hooks allow shell commands to run at specific points in the tool
//! execution lifecycle (e.g. before/after a tool call, on session start).

use cc_error::{CcError, CcResult};
use serde::{Deserialize, Serialize};

// ── Types ───────────────────────────────────────────────────────────

/// When a hook fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookTiming {
    /// Before a tool is executed.
    PreToolUse,
    /// After a tool has executed.
    PostToolUse,
    /// When a new session starts.
    SessionStart,
}

/// A single hook definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hook {
    /// A human-readable name for this hook.
    pub name: String,
    /// When this hook should fire.
    pub timing: HookTiming,
    /// Optional tool name filter (if `None`, applies to all tools).
    pub tool_filter: Option<String>,
    /// The shell command to execute.
    pub command: String,
}

/// The outcome of running a pre-tool hook.
#[derive(Debug, Clone)]
pub enum HookResult {
    /// Allow the tool call to proceed.
    Allow,
    /// Deny the tool call with a reason.
    Deny(String),
    /// Modify the tool input before execution.
    Modify(serde_json::Value),
}

// ── Registry ────────────────────────────────────────────────────────

/// Holds all registered hooks and dispatches them at the right time.
pub struct HookRegistry {
    hooks: Vec<Hook>,
}

impl HookRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { hooks: Vec::new() }
    }

    /// Register a hook.
    pub fn register(&mut self, hook: Hook) {
        tracing::debug!(name = %hook.name, timing = ?hook.timing, "registered hook");
        self.hooks.push(hook);
    }

    /// Return all registered hooks.
    pub fn list(&self) -> &[Hook] {
        &self.hooks
    }

    /// Run all `PreToolUse` hooks that match the given tool name.
    ///
    /// Returns the first non-Allow result, or `Allow` if all hooks pass.
    pub async fn run_pre_tool(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> HookResult {
        for hook in &self.hooks {
            if hook.timing != HookTiming::PreToolUse {
                continue;
            }
            if let Some(ref filter) = hook.tool_filter {
                if filter != tool_name {
                    continue;
                }
            }

            match self.execute_hook(hook, tool_name, Some(input)).await {
                Ok(result) => match result {
                    HookResult::Allow => continue,
                    other => return other,
                },
                Err(e) => {
                    tracing::warn!(hook = %hook.name, error = %e, "pre-tool hook failed");
                    return HookResult::Deny(format!("Hook '{}' failed: {}", hook.name, e));
                }
            }
        }

        HookResult::Allow
    }

    /// Run all `PostToolUse` hooks that match the given tool name.
    pub async fn run_post_tool(&self, tool_name: &str, output: &str) {
        for hook in &self.hooks {
            if hook.timing != HookTiming::PostToolUse {
                continue;
            }
            if let Some(ref filter) = hook.tool_filter {
                if filter != tool_name {
                    continue;
                }
            }

            let input_val = serde_json::json!({ "output": output });
            if let Err(e) = self.execute_hook(hook, tool_name, Some(&input_val)).await {
                tracing::warn!(hook = %hook.name, error = %e, "post-tool hook failed");
            }
        }
    }

    /// Run all `SessionStart` hooks.
    pub async fn run_session_start(&self) {
        for hook in &self.hooks {
            if hook.timing != HookTiming::SessionStart {
                continue;
            }
            if let Err(e) = self.execute_hook(hook, "", None).await {
                tracing::warn!(hook = %hook.name, error = %e, "session-start hook failed");
            }
        }
    }

    /// Execute a single hook by running its shell command.
    async fn execute_hook(
        &self,
        hook: &Hook,
        tool_name: &str,
        input: Option<&serde_json::Value>,
    ) -> CcResult<HookResult> {
        tracing::debug!(hook = %hook.name, tool = tool_name, "executing hook");

        let input_json = input
            .map(|v| serde_json::to_string(v).unwrap_or_default())
            .unwrap_or_default();

        let output = tokio::process::Command::new(shell_program())
            .arg(shell_flag())
            .arg(&hook.command)
            .env("CC_TOOL_NAME", tool_name)
            .env("CC_TOOL_INPUT", &input_json)
            .output()
            .await
            .map_err(|e| CcError::Internal(format!("failed to run hook '{}': {}", hook.name, e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            // Check if the hook explicitly denied the call.
            let combined = format!("{}{}", stdout, stderr);
            if combined.contains("DENY:") {
                let reason = combined
                    .lines()
                    .find(|l| l.starts_with("DENY:"))
                    .map(|l| l.trim_start_matches("DENY:").trim().to_string())
                    .unwrap_or_else(|| "hook denied".into());
                return Ok(HookResult::Deny(reason));
            }

            return Ok(HookResult::Deny(format!(
                "hook '{}' exited with status {}",
                hook.name,
                output.status
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Check if the hook wants to modify the input.
        if let Some(line) = stdout.lines().find(|l| l.starts_with("MODIFY:")) {
            let json_str = line.trim_start_matches("MODIFY:").trim();
            if let Ok(val) = serde_json::from_str(json_str) {
                return Ok(HookResult::Modify(val));
            }
        }

        Ok(HookResult::Allow)
    }
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the platform-appropriate shell program.
fn shell_program() -> &'static str {
    if cfg!(windows) { "cmd" } else { "sh" }
}

/// Returns the flag to pass a command string to the shell.
fn shell_flag() -> &'static str {
    if cfg!(windows) { "/C" } else { "-c" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_basics() {
        let mut reg = HookRegistry::new();
        assert!(reg.list().is_empty());

        reg.register(Hook {
            name: "test".into(),
            timing: HookTiming::PreToolUse,
            tool_filter: None,
            command: "echo ok".into(),
        });

        assert_eq!(reg.list().len(), 1);
    }

    #[tokio::test]
    async fn pre_tool_with_no_hooks() {
        let reg = HookRegistry::new();
        let result = reg
            .run_pre_tool("bash", &serde_json::json!({"command": "ls"}))
            .await;
        assert!(matches!(result, HookResult::Allow));
    }

    #[test]
    fn hook_timing_serialization() {
        let json = serde_json::to_string(&HookTiming::PreToolUse).unwrap();
        assert_eq!(json, "\"pre_tool_use\"");

        let deserialized: HookTiming = serde_json::from_str("\"post_tool_use\"").unwrap();
        assert_eq!(deserialized, HookTiming::PostToolUse);
    }
}
