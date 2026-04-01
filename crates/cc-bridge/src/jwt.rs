//! JWT token handling for bridge authentication.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use cc_error::{CcError, CcResult};

use crate::config::BridgeConfig;

/// A JWT token pair with expiry tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtToken {
    /// The access token used for API requests.
    pub access_token: String,
    /// An optional refresh token for obtaining new access tokens.
    pub refresh_token: Option<String>,
    /// When the access token expires.
    pub expires_at: DateTime<Utc>,
}

impl JwtToken {
    /// Returns `true` if the access token has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.expires_at
    }

    /// Decode the claims (payload) section of a JWT token.
    ///
    /// This performs a base64 decode of the middle segment without
    /// verifying the signature -- suitable for inspecting token metadata.
    pub fn decode_claims(token: &str) -> CcResult<serde_json::Value> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(CcError::Auth("Invalid JWT: expected 3 dot-separated parts".into()));
        }

        let payload_bytes = URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|e| CcError::Auth(format!("Failed to base64-decode JWT payload: {e}")))?;

        serde_json::from_slice(&payload_bytes)
            .map_err(|e| CcError::Serialization(format!("Failed to parse JWT claims: {e}")))
    }

    /// Returns the duration remaining before the token expires.
    /// Returns a zero duration if the token is already expired.
    pub fn time_until_expiry(&self) -> Duration {
        let remaining = self.expires_at - Utc::now();
        if remaining < Duration::zero() {
            Duration::zero()
        } else {
            remaining
        }
    }
}

/// Handles refreshing JWT tokens before they expire.
pub struct TokenRefresher {
    config: BridgeConfig,
}

impl TokenRefresher {
    /// Create a new `TokenRefresher` with the given bridge configuration.
    pub fn new(config: BridgeConfig) -> Self {
        Self { config }
    }

    /// Refresh the given token by calling the bridge API.
    pub async fn refresh(&self, token: &JwtToken) -> CcResult<JwtToken> {
        let refresh_token = token
            .refresh_token
            .as_deref()
            .ok_or_else(|| CcError::Auth("No refresh token available".into()))?;

        let client = reqwest::Client::new();
        let url = format!("{}/auth/refresh", self.config.api_url);

        let resp = client
            .post(&url)
            .json(&serde_json::json!({ "refresh_token": refresh_token }))
            .send()
            .await
            .map_err(|e| CcError::Api {
                message: format!("Token refresh request failed: {e}"),
                status_code: None,
            })?;

        if !resp.status().is_success() {
            return Err(CcError::Auth(format!(
                "Token refresh failed with status {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            CcError::Serialization(format!("Failed to parse refresh response: {e}"))
        })?;

        let access_token = body["access_token"]
            .as_str()
            .ok_or_else(|| CcError::Auth("Missing access_token in refresh response".into()))?
            .to_string();

        let new_refresh = body["refresh_token"].as_str().map(|s| s.to_string());
        let expires_in = body["expires_in"].as_i64().unwrap_or(3600);

        Ok(JwtToken {
            access_token,
            refresh_token: new_refresh.or_else(|| token.refresh_token.clone()),
            expires_at: Utc::now() + Duration::seconds(expires_in),
        })
    }

    /// Returns `true` if the token should be proactively refreshed.
    ///
    /// Triggers a refresh when 80% of the token's lifetime has elapsed.
    pub fn should_refresh(&self, token: &JwtToken) -> bool {
        let remaining = token.time_until_expiry();
        let total_secs = (token.expires_at - Utc::now() + remaining).num_seconds();
        if total_secs <= 0 {
            return true;
        }
        // Refresh when less than 20% of lifetime remains.
        remaining.num_seconds() < (total_secs / 5).max(30)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_token_is_detected() {
        let token = JwtToken {
            access_token: "test".into(),
            refresh_token: None,
            expires_at: Utc::now() - Duration::seconds(60),
        };
        assert!(token.is_expired());
    }

    #[test]
    fn valid_token_is_not_expired() {
        let token = JwtToken {
            access_token: "test".into(),
            refresh_token: None,
            expires_at: Utc::now() + Duration::hours(1),
        };
        assert!(!token.is_expired());
    }

    #[test]
    fn decode_claims_rejects_malformed_jwt() {
        assert!(JwtToken::decode_claims("not-a-jwt").is_err());
        assert!(JwtToken::decode_claims("a.b").is_err());
    }

    #[test]
    fn decode_claims_parses_valid_payload() {
        // Build a minimal JWT with a JSON payload in the middle segment.
        let payload = serde_json::json!({"sub": "user123", "exp": 9999999999_u64});
        let encoded = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&payload).unwrap());
        let fake_jwt = format!("header.{encoded}.signature");

        let claims = JwtToken::decode_claims(&fake_jwt).unwrap();
        assert_eq!(claims["sub"], "user123");
    }

    #[test]
    fn time_until_expiry_returns_zero_for_expired() {
        let token = JwtToken {
            access_token: "x".into(),
            refresh_token: None,
            expires_at: Utc::now() - Duration::hours(1),
        };
        assert_eq!(token.time_until_expiry(), Duration::zero());
    }

    #[test]
    fn test_jwt_expiry_check() {
        // Token that expired 10 seconds ago should be expired.
        let expired = JwtToken {
            access_token: "expired-tok".into(),
            refresh_token: Some("refresh-tok".into()),
            expires_at: Utc::now() - Duration::seconds(10),
        };
        assert!(expired.is_expired());
        assert_eq!(expired.time_until_expiry(), Duration::zero());

        // Token expiring in 1 hour should not be expired.
        let fresh = JwtToken {
            access_token: "fresh-tok".into(),
            refresh_token: None,
            expires_at: Utc::now() + Duration::hours(1),
        };
        assert!(!fresh.is_expired());
        assert!(fresh.time_until_expiry().num_seconds() > 3500);

        // Token exactly at the boundary (expires_at == now) should be expired
        // since the check is >=.
        let boundary = JwtToken {
            access_token: "boundary-tok".into(),
            refresh_token: None,
            expires_at: Utc::now(),
        };
        assert!(boundary.is_expired());
    }
}
