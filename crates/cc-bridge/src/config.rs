//! Bridge configuration.

use serde::{Deserialize, Serialize};

/// Configuration for connecting to a Claude bridge server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// The base HTTP API URL for the bridge server.
    pub api_url: String,
    /// The WebSocket URL for real-time communication.
    pub ws_url: String,
    /// A unique identifier for this device/client.
    pub device_id: String,
    /// An optional session token for resuming sessions.
    pub session_token: Option<String>,
}

impl BridgeConfig {
    /// Create a new `BridgeConfig` with default claude.ai URLs.
    pub fn default_urls() -> Self {
        Self {
            api_url: "https://claude.ai/api".to_string(),
            ws_url: "wss://claude.ai/ws".to_string(),
            device_id: String::new(),
            session_token: None,
        }
    }
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self::default_urls()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_urls_are_set() {
        let config = BridgeConfig::default_urls();
        assert!(config.api_url.starts_with("https://"));
        assert!(config.ws_url.starts_with("wss://"));
        assert!(config.session_token.is_none());
    }
}
