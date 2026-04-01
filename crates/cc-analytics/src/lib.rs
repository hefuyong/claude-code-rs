//! Analytics and telemetry for Claude Code RS.
//!
//! Provides a client for tracking usage events. Currently a stub
//! that only logs events locally.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
