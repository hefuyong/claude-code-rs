//! Analytics, telemetry, feature flags, and policy limits for Claude Code RS.
//!
//! Provides:
//! - [`AnalyticsClient`] -- event tracking (currently a local stub).
//! - [`FeatureFlags`]    -- boolean feature gates with override support.
//! - [`PolicyLimits`]    -- per-session resource and access limits.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// An analytics event to track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsEvent {
    /// Event name (e.g. "session_start", "tool_use").
    pub name: String,
    /// Arbitrary event data.
    pub data: serde_json::Value,
    /// Timestamp (Unix millis).
    pub timestamp_ms: u64,
}

/// Client for sending analytics events.
pub struct AnalyticsClient {
    /// Whether analytics are enabled.
    enabled: bool,
    /// Buffered events (flushed periodically or on shutdown).
    buffer: Vec<AnalyticsEvent>,
}

impl AnalyticsClient {
    /// Create a new analytics client.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            buffer: Vec::new(),
        }
    }

    /// Track an event with the given name and data.
    pub fn track_event(&mut self, name: &str, data: serde_json::Value) {
        if !self.enabled {
            return;
        }

        let event = AnalyticsEvent {
            name: name.to_string(),
            data,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        };

        tracing::trace!(event_name = name, "analytics event tracked");
        self.buffer.push(event);
    }

    /// Return the number of buffered events.
    pub fn buffered_count(&self) -> usize {
        self.buffer.len()
    }

    /// Flush the event buffer (currently just clears it).
    pub fn flush(&mut self) {
        tracing::debug!(count = self.buffer.len(), "flushing analytics buffer");
        self.buffer.clear();
    }

    /// Whether analytics collection is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

impl Default for AnalyticsClient {
    fn default() -> Self {
        Self::new(false)
    }
}

// ---------------------------------------------------------------------------
// Default feature flags
// ---------------------------------------------------------------------------

/// Built-in feature flag defaults.
const DEFAULT_FLAGS: &[(&str, bool)] = &[
    ("voice_mode", false),
    ("coordinator_mode", false),
    ("dream_tasks", false),
    ("auto_memory", true),
    ("auto_compact", true),
    ("lsp_integration", false),
    ("ide_connect", false),
    ("bridge_mode", false),
    ("output_styles", true),
    ("custom_keybindings", true),
];

// ---------------------------------------------------------------------------
// FeatureFlags
// ---------------------------------------------------------------------------

/// Boolean feature gates with runtime override support.
///
/// Flags can be loaded from a config file and individually overridden
/// at runtime (e.g. via CLI flags or environment variables).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    flags: HashMap<String, bool>,
    overrides: HashMap<String, bool>,
}

impl FeatureFlags {
    /// Create an empty feature-flag set.
    pub fn new() -> Self {
        Self {
            flags: HashMap::new(),
            overrides: HashMap::new(),
        }
    }

    /// Set a flag value (base layer, before overrides).
    pub fn set(&mut self, flag: &str, value: bool) {
        self.flags.insert(flag.to_string(), value);
    }

    /// Check whether a flag is enabled.
    ///
    /// Overrides take precedence, then the base flags map, then `false`.
    pub fn is_enabled(&self, flag: &str) -> bool {
        if let Some(&v) = self.overrides.get(flag) {
            return v;
        }
        self.flags.get(flag).copied().unwrap_or(false)
    }

    /// Set a runtime override for a flag.
    pub fn override_flag(&mut self, flag: &str, value: bool) {
        self.overrides.insert(flag.to_string(), value);
    }

    /// Remove a runtime override, reverting to the base value.
    pub fn clear_override(&mut self, flag: &str) {
        self.overrides.remove(flag);
    }

    /// Return all base flags (without overrides applied).
    pub fn all_flags(&self) -> &HashMap<String, bool> {
        &self.flags
    }

    /// Populate from a JSON config object.
    ///
    /// Expects the config to contain a `"feature_flags"` key with an
    /// object of `{ "flag_name": true/false }` entries.
    pub fn load_from_config(config: &serde_json::Value) -> Self {
        let mut ff = Self::new();
        ff.register_defaults();

        if let Some(obj) = config.get("feature_flags").and_then(|v| v.as_object()) {
            for (key, val) in obj {
                if let Some(b) = val.as_bool() {
                    ff.set(key, b);
                }
            }
        }
        ff
    }

    /// Register all built-in default flags.
    pub fn register_defaults(&mut self) {
        for &(name, value) in DEFAULT_FLAGS {
            self.flags.entry(name.to_string()).or_insert(value);
        }
    }
}

impl Default for FeatureFlags {
    fn default() -> Self {
        let mut ff = Self::new();
        ff.register_defaults();
        ff
    }
}

// ---------------------------------------------------------------------------
// PolicyLimits
// ---------------------------------------------------------------------------

/// Per-session resource and access limits, typically loaded from an
/// organization or project policy file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyLimits {
    /// Maximum total tokens consumed in a session.
    pub max_tokens_per_session: Option<u64>,
    /// Maximum cost (in USD) for a session.
    pub max_cost_per_session: Option<f64>,
    /// Maximum agentic turns per user query.
    pub max_turns_per_query: Option<u32>,
    /// Allow-list of model identifiers.
    pub allowed_models: Option<Vec<String>>,
    /// Allow-list of tool names.
    pub allowed_tools: Option<Vec<String>>,
}

impl PolicyLimits {
    /// Load limits from a JSON config object.
    ///
    /// Expects keys like `max_tokens_per_session`, `max_cost_per_session`,
    /// etc. at the top level of the config.
    pub fn from_config(config: &serde_json::Value) -> Self {
        Self {
            max_tokens_per_session: config
                .get("max_tokens_per_session")
                .and_then(|v| v.as_u64()),
            max_cost_per_session: config
                .get("max_cost_per_session")
                .and_then(|v| v.as_f64()),
            max_turns_per_query: config
                .get("max_turns_per_query")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32),
            allowed_models: config
                .get("allowed_models")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
            allowed_tools: config
                .get("allowed_tools")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                }),
        }
    }

    /// Returns `true` if `current` tokens are within the limit (or no limit is set).
    pub fn check_token_limit(&self, current: u64) -> bool {
        self.max_tokens_per_session
            .map_or(true, |max| current <= max)
    }

    /// Returns `true` if `current` cost is within the limit (or no limit is set).
    pub fn check_cost_limit(&self, current: f64) -> bool {
        self.max_cost_per_session
            .map_or(true, |max| current <= max)
    }

    /// Returns `true` if `model` is allowed (or no allow-list is set).
    pub fn check_model_allowed(&self, model: &str) -> bool {
        self.allowed_models.as_ref().map_or(true, |models| {
            models.iter().any(|m| m == model)
        })
    }

    /// Returns `true` if `tool` is allowed (or no allow-list is set).
    pub fn check_tool_allowed(&self, tool: &str) -> bool {
        self.allowed_tools.as_ref().map_or(true, |tools| {
            tools.iter().any(|t| t == tool)
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- AnalyticsClient tests ---------------------------------------------

    #[test]
    fn disabled_client_ignores_events() {
        let mut client = AnalyticsClient::new(false);
        client.track_event("test", serde_json::json!({"key": "value"}));
        assert_eq!(client.buffered_count(), 0);
    }

    #[test]
    fn enabled_client_buffers_events() {
        let mut client = AnalyticsClient::new(true);
        client.track_event("test", serde_json::json!({}));
        client.track_event("test2", serde_json::json!({}));
        assert_eq!(client.buffered_count(), 2);
    }

    #[test]
    fn flush_clears_buffer() {
        let mut client = AnalyticsClient::new(true);
        client.track_event("test", serde_json::json!({}));
        client.flush();
        assert_eq!(client.buffered_count(), 0);
    }

    // -- FeatureFlags tests ------------------------------------------------

    #[test]
    fn test_defaults_registered() {
        let ff = FeatureFlags::default();
        assert!(ff.is_enabled("auto_memory"));
        assert!(ff.is_enabled("auto_compact"));
        assert!(!ff.is_enabled("voice_mode"));
        assert!(!ff.is_enabled("coordinator_mode"));
    }

    #[test]
    fn test_override_takes_precedence() {
        let mut ff = FeatureFlags::default();
        assert!(!ff.is_enabled("voice_mode"));
        ff.override_flag("voice_mode", true);
        assert!(ff.is_enabled("voice_mode"));

        ff.clear_override("voice_mode");
        assert!(!ff.is_enabled("voice_mode"));
    }

    #[test]
    fn test_unknown_flag_defaults_false() {
        let ff = FeatureFlags::default();
        assert!(!ff.is_enabled("totally_unknown"));
    }

    #[test]
    fn test_load_from_config() {
        let config = serde_json::json!({
            "feature_flags": {
                "voice_mode": true,
                "auto_memory": false,
                "custom_flag": true
            }
        });
        let ff = FeatureFlags::load_from_config(&config);
        assert!(ff.is_enabled("voice_mode"));
        assert!(!ff.is_enabled("auto_memory"));
        assert!(ff.is_enabled("custom_flag"));
        // Defaults still present
        assert!(ff.is_enabled("auto_compact"));
    }

    #[test]
    fn test_feature_flags_set_and_get() {
        let mut ff = FeatureFlags::new();
        ff.set("my_flag", true);
        assert!(ff.is_enabled("my_flag"));
        ff.set("my_flag", false);
        assert!(!ff.is_enabled("my_flag"));
    }

    // -- PolicyLimits tests ------------------------------------------------

    #[test]
    fn test_no_limits_allows_everything() {
        let pl = PolicyLimits::default();
        assert!(pl.check_token_limit(999_999_999));
        assert!(pl.check_cost_limit(999.99));
        assert!(pl.check_model_allowed("any-model"));
        assert!(pl.check_tool_allowed("any-tool"));
    }

    #[test]
    fn test_token_limit() {
        let pl = PolicyLimits {
            max_tokens_per_session: Some(1000),
            ..Default::default()
        };
        assert!(pl.check_token_limit(500));
        assert!(pl.check_token_limit(1000));
        assert!(!pl.check_token_limit(1001));
    }

    #[test]
    fn test_cost_limit() {
        let pl = PolicyLimits {
            max_cost_per_session: Some(5.0),
            ..Default::default()
        };
        assert!(pl.check_cost_limit(4.99));
        assert!(!pl.check_cost_limit(5.01));
    }

    #[test]
    fn test_model_allow_list() {
        let pl = PolicyLimits {
            allowed_models: Some(vec![
                "claude-opus-4-20250514".into(),
                "claude-sonnet-4-20250514".into(),
            ]),
            ..Default::default()
        };
        assert!(pl.check_model_allowed("claude-opus-4-20250514"));
        assert!(!pl.check_model_allowed("gpt-4"));
    }

    #[test]
    fn test_tool_allow_list() {
        let pl = PolicyLimits {
            allowed_tools: Some(vec!["bash".into(), "read".into()]),
            ..Default::default()
        };
        assert!(pl.check_tool_allowed("bash"));
        assert!(!pl.check_tool_allowed("write"));
    }

    #[test]
    fn test_policy_from_config() {
        let config = serde_json::json!({
            "max_tokens_per_session": 50000,
            "max_cost_per_session": 10.0,
            "max_turns_per_query": 25,
            "allowed_models": ["claude-opus-4-20250514"],
            "allowed_tools": ["bash", "read", "write"]
        });
        let pl = PolicyLimits::from_config(&config);
        assert_eq!(pl.max_tokens_per_session, Some(50000));
        assert_eq!(pl.max_cost_per_session, Some(10.0));
        assert_eq!(pl.max_turns_per_query, Some(25));
        assert_eq!(pl.allowed_models.as_ref().unwrap().len(), 1);
        assert_eq!(pl.allowed_tools.as_ref().unwrap().len(), 3);
    }
}
