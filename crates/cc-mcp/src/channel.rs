//! MCP Channel management.
//!
//! Controls which MCP servers are allowed or denied, and buffers
//! per-server notifications so that the UI can display them.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ChannelNotification
// ---------------------------------------------------------------------------

/// A notification received from an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelNotification {
    /// The server that sent the notification.
    pub server_name: String,
    /// The notification message body.
    pub message: String,
    /// When the notification was received.
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// ChannelManager
// ---------------------------------------------------------------------------

/// Manages MCP server allow/deny lists and notification buffers.
pub struct ChannelManager {
    /// Servers explicitly allowed to communicate.
    allowed: HashSet<String>,
    /// Servers explicitly denied.
    denied: HashSet<String>,
    /// Per-server notification log.
    notifications: HashMap<String, Vec<ChannelNotification>>,
}

impl ChannelManager {
    /// Create an empty channel manager.
    pub fn new() -> Self {
        Self {
            allowed: HashSet::new(),
            denied: HashSet::new(),
            notifications: HashMap::new(),
        }
    }

    /// Mark a server as explicitly allowed.
    pub fn allow_server(&mut self, name: &str) {
        self.denied.remove(name);
        self.allowed.insert(name.to_string());
    }

    /// Mark a server as explicitly denied.
    pub fn deny_server(&mut self, name: &str) {
        self.allowed.remove(name);
        self.denied.insert(name.to_string());
    }

    /// Check whether a server is allowed.
    ///
    /// Deny-list takes precedence. If neither list contains the server,
    /// it is allowed by default.
    pub fn is_allowed(&self, name: &str) -> bool {
        if self.denied.contains(name) {
            return false;
        }
        // If there's an allow-list at all, the server must be in it.
        if !self.allowed.is_empty() {
            return self.allowed.contains(name);
        }
        true
    }

    /// Buffer a notification from a server.
    pub fn add_notification(&mut self, server: &str, message: &str) {
        let notif = ChannelNotification {
            server_name: server.to_string(),
            message: message.to_string(),
            timestamp: Utc::now(),
        };
        self.notifications
            .entry(server.to_string())
            .or_default()
            .push(notif);
    }

    /// Return all notifications for a given server.
    pub fn get_notifications(&self, server: &str) -> Vec<&ChannelNotification> {
        self.notifications
            .get(server)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Clear all notifications for a server.
    pub fn clear_notifications(&mut self, server: &str) {
        self.notifications.remove(server);
    }

    /// Total number of buffered notifications across all servers.
    pub fn total_notification_count(&self) -> usize {
        self.notifications.values().map(|v| v.len()).sum()
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_allows_all() {
        let cm = ChannelManager::new();
        assert!(cm.is_allowed("any-server"));
    }

    #[test]
    fn test_deny_blocks_server() {
        let mut cm = ChannelManager::new();
        cm.deny_server("evil-server");
        assert!(!cm.is_allowed("evil-server"));
        assert!(cm.is_allowed("good-server"));
    }

    #[test]
    fn test_allow_list_restricts() {
        let mut cm = ChannelManager::new();
        cm.allow_server("only-this");
        // An allow-list is present, so unlisted servers are blocked.
        assert!(cm.is_allowed("only-this"));
        assert!(!cm.is_allowed("other-server"));
    }

    #[test]
    fn test_deny_overrides_allow() {
        let mut cm = ChannelManager::new();
        cm.allow_server("server-x");
        cm.deny_server("server-x");
        assert!(!cm.is_allowed("server-x"));
    }

    #[test]
    fn test_notifications() {
        let mut cm = ChannelManager::new();
        cm.add_notification("server-a", "hello");
        cm.add_notification("server-a", "world");
        cm.add_notification("server-b", "ping");

        assert_eq!(cm.get_notifications("server-a").len(), 2);
        assert_eq!(cm.get_notifications("server-b").len(), 1);
        assert_eq!(cm.get_notifications("server-c").len(), 0);
        assert_eq!(cm.total_notification_count(), 3);
    }

    #[test]
    fn test_clear_notifications() {
        let mut cm = ChannelManager::new();
        cm.add_notification("s", "msg1");
        cm.add_notification("s", "msg2");
        cm.clear_notifications("s");
        assert_eq!(cm.get_notifications("s").len(), 0);
        assert_eq!(cm.total_notification_count(), 0);
    }
}
