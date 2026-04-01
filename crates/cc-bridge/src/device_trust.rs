//! Device trust management for the bridge.
//!
//! Tracks a device identity derived from machine-specific information
//! and manages trust tokens obtained from the bridge server.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use cc_error::{CcError, CcResult};

use crate::config::BridgeConfig;

/// Represents trust state for the current device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceTrust {
    /// A stable identifier for this device, derived from machine info.
    pub device_id: String,
    /// A trust token granted by the bridge server after verification.
    pub trust_token: Option<String>,
    /// When the device was last trusted.
    pub trusted_at: Option<DateTime<Utc>>,
}

impl DeviceTrust {
    /// Create a new `DeviceTrust` with a device ID derived from the hostname.
    pub fn new() -> Self {
        let hostname = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "unknown-host".to_string());

        let mut hasher = Sha256::new();
        hasher.update(hostname.as_bytes());
        hasher.update(b"claude-code-rs");
        let hash = hasher.finalize();
        let device_id = format!("{:x}", hash);

        Self {
            device_id,
            trust_token: None,
            trusted_at: None,
        }
    }

    /// Returns `true` if this device has a valid trust token.
    pub fn is_trusted(&self) -> bool {
        self.trust_token.is_some()
    }

    /// Request a trust token from the bridge server.
    pub async fn request_trust(&mut self, config: &BridgeConfig) -> CcResult<()> {
        let client = reqwest::Client::new();
        let url = format!("{}/devices/trust", config.api_url);

        let resp = client
            .post(&url)
            .json(&serde_json::json!({ "device_id": self.device_id }))
            .send()
            .await
            .map_err(|e| CcError::Api {
                message: format!("Device trust request failed: {e}"),
                status_code: None,
            })?;

        if !resp.status().is_success() {
            return Err(CcError::Auth(format!(
                "Device trust request denied with status {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            CcError::Serialization(format!("Failed to parse trust response: {e}"))
        })?;

        self.trust_token = body["trust_token"].as_str().map(|s| s.to_string());
        self.trusted_at = Some(Utc::now());
        Ok(())
    }

    /// Load device trust state from a JSON file.
    pub fn load(path: &Path) -> CcResult<Self> {
        let data = std::fs::read_to_string(path).map_err(CcError::Io)?;
        serde_json::from_str(&data)
            .map_err(|e| CcError::Serialization(format!("Failed to load device trust: {e}")))
    }

    /// Save device trust state to a JSON file.
    pub fn save(&self, path: &Path) -> CcResult<()> {
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| CcError::Serialization(format!("Failed to serialize device trust: {e}")))?;
        std::fs::write(path, data).map_err(CcError::Io)
    }
}

impl Default for DeviceTrust {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn new_device_is_not_trusted() {
        let device = DeviceTrust::new();
        assert!(!device.is_trusted());
        assert!(!device.device_id.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let mut device = DeviceTrust::new();
        device.trust_token = Some("tok_test".into());
        device.trusted_at = Some(Utc::now());

        let tmp = NamedTempFile::new().unwrap();
        device.save(tmp.path()).unwrap();

        let loaded = DeviceTrust::load(tmp.path()).unwrap();
        assert_eq!(loaded.device_id, device.device_id);
        assert_eq!(loaded.trust_token, Some("tok_test".into()));
        assert!(loaded.is_trusted());
    }

    #[test]
    fn test_device_id_deterministic() {
        // Two DeviceTrust instances created on the same machine should
        // produce the same device_id since it is derived from
        // hostname + salt.
        let d1 = DeviceTrust::new();
        let d2 = DeviceTrust::new();
        assert_eq!(d1.device_id, d2.device_id);
        // The device ID should be a hex-encoded SHA-256 hash (64 chars).
        assert_eq!(d1.device_id.len(), 64);
        assert!(d1.device_id.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
