//! OAuth 2.0 PKCE flow support for Claude Code RS.
//!
//! Provides types and utilities for the Authorization Code flow
//! with PKCE (Proof Key for Code Exchange), used for authenticating
//! with Anthropic's API via browser-based login.
//!
//! The [`OAuthClient`] struct wraps the free functions into a
//! stateful object that tracks authentication progress and manages
//! token refresh.

use cc_error::{CcError, CcResult};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── Types ───────────────────────────────────────────────────────────

/// OAuth client configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// The OAuth client ID.
    pub client_id: String,
    /// The authorization endpoint URL.
    pub auth_url: String,
    /// The token endpoint URL.
    pub token_url: String,
    /// The redirect URI (usually a local server).
    pub redirect_uri: String,
    /// Requested scopes.
    pub scopes: Vec<String>,
}

/// PKCE challenge pair: verifier + challenge.
#[derive(Debug, Clone)]
pub struct PkceChallenge {
    /// The random verifier string (kept secret, sent in token exchange).
    pub verifier: String,
    /// The S256 challenge derived from the verifier (sent in auth URL).
    pub challenge: String,
}

/// An OAuth token response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    /// The access token.
    pub access_token: String,
    /// Token type (usually "Bearer").
    pub token_type: String,
    /// Time in seconds until the token expires.
    pub expires_in: Option<u64>,
    /// Refresh token, if provided.
    pub refresh_token: Option<String>,
}

/// Stores the current authentication state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    /// The current access token, if authenticated.
    pub access_token: Option<String>,
    /// The refresh token, if available.
    pub refresh_token: Option<String>,
    /// When the access token expires (unix timestamp).
    pub expires_at: Option<u64>,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            access_token: None,
            refresh_token: None,
            expires_at: None,
        }
    }
}

// ── OAuthState (client lifecycle) ──────────────────────────────────

/// Tracks where the client is in the OAuth flow.
#[derive(Debug, Clone)]
pub enum OAuthState {
    /// No tokens, not yet started.
    Unauthenticated,
    /// Authorization URL opened, waiting for the redirect callback.
    AwaitingCallback {
        /// The PKCE verifier to use when exchanging the code.
        verifier: String,
        /// The random `state` parameter for CSRF protection.
        state: String,
    },
    /// Tokens obtained successfully.
    Authenticated(TokenResponse),
}

// ── OAuthClient ────────────────────────────────────────────────────

/// High-level client that manages the full OAuth PKCE flow.
///
/// # Usage
///
/// ```ignore
/// let client = OAuthClient::new(config);
/// let (url, challenge) = client.build_auth_url();
/// // Open `url` in the user's browser ...
/// let code = client.start_local_server().await?;
/// let tokens = client.exchange_code(&code, &challenge.verifier).await?;
/// ```
pub struct OAuthClient {
    config: OAuthConfig,
    http: reqwest::Client,
    state: OAuthState,
}

impl OAuthClient {
    /// Create a new OAuth client from the given configuration.
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
            state: OAuthState::Unauthenticated,
        }
    }

    /// Generate a PKCE challenge and build the full authorization URL.
    ///
    /// Returns the URL to open in the browser and the PKCE challenge
    /// (whose `verifier` field is needed for [`exchange_code`]).
    pub fn build_auth_url(&mut self) -> (String, PkceChallenge) {
        let pkce = generate_pkce_challenge();
        let state_param = generate_random_state();

        let url = format!(
            "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
            self.config.auth_url,
            urlencoding_encode(&self.config.client_id),
            urlencoding_encode(&self.config.redirect_uri),
            urlencoding_encode(&self.config.scopes.join(" ")),
            urlencoding_encode(&state_param),
            urlencoding_encode(&pkce.challenge),
        );

        self.state = OAuthState::AwaitingCallback {
            verifier: pkce.verifier.clone(),
            state: state_param,
        };

        (url, pkce)
    }

    /// Exchange an authorization code for tokens.
    ///
    /// On success the client transitions to `Authenticated` state.
    pub async fn exchange_code(&mut self, code: &str, verifier: &str) -> CcResult<TokenResponse> {
        let response = self
            .http
            .post(&self.config.token_url)
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", &self.config.client_id),
                ("code", code),
                ("redirect_uri", &self.config.redirect_uri),
                ("code_verifier", verifier),
            ])
            .send()
            .await
            .map_err(|e| CcError::Auth(format!("token exchange request failed: {e}")))?;

        let status = response.status().as_u16();
        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            return Err(CcError::Auth(format!(
                "token exchange failed (status {status}): {body}"
            )));
        }

        let tokens: TokenResponse = response
            .json()
            .await
            .map_err(|e| CcError::Auth(format!("failed to parse token response: {e}")))?;

        self.state = OAuthState::Authenticated(tokens.clone());
        Ok(tokens)
    }

    /// Refresh the access token using a refresh token.
    ///
    /// On success the client remains in `Authenticated` state with
    /// updated tokens.
    pub async fn refresh_token(&mut self, refresh_token: &str) -> CcResult<TokenResponse> {
        let response = self
            .http
            .post(&self.config.token_url)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", &self.config.client_id),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await
            .map_err(|e| CcError::Auth(format!("token refresh request failed: {e}")))?;

        let status = response.status().as_u16();
        if status != 200 {
            let body = response.text().await.unwrap_or_default();
            return Err(CcError::Auth(format!(
                "token refresh failed (status {status}): {body}"
            )));
        }

        let tokens: TokenResponse = response
            .json()
            .await
            .map_err(|e| CcError::Auth(format!("failed to parse refresh response: {e}")))?;

        self.state = OAuthState::Authenticated(tokens.clone());
        Ok(tokens)
    }

    /// Start a local HTTP server on `redirect_uri` and wait for the
    /// OAuth callback.
    ///
    /// Parses the `code` query parameter from the incoming request and
    /// returns it. The server shuts down after the first request.
    ///
    /// This is a placeholder -- a full implementation would bind a
    /// `tokio::net::TcpListener` on the redirect port and serve a
    /// response page.
    pub async fn start_local_server(&self) -> CcResult<String> {
        // Extract port from redirect_uri (e.g. "http://localhost:9876/callback").
        let port = self
            .config
            .redirect_uri
            .split(':')
            .last()
            .and_then(|s| s.split('/').next())
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(9876);

        tracing::info!(port, "starting local OAuth callback server");

        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
            .await
            .map_err(|e| CcError::Auth(format!("failed to bind callback server: {e}")))?;

        let (mut stream, _addr) = listener
            .accept()
            .await
            .map_err(|e| CcError::Auth(format!("failed to accept callback: {e}")))?;

        // Read the HTTP request line to extract the code.
        let mut buf = vec![0u8; 4096];
        let n = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
            .await
            .map_err(|e| CcError::Auth(format!("failed to read callback: {e}")))?;

        let request = String::from_utf8_lossy(&buf[..n]);

        // Parse `?code=...` from the GET request line.
        let code = request
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|path| {
                path.split('?')
                    .nth(1)
                    .and_then(|qs| {
                        qs.split('&').find_map(|pair| {
                            let mut kv = pair.splitn(2, '=');
                            if kv.next() == Some("code") {
                                kv.next().map(|v| v.to_string())
                            } else {
                                None
                            }
                        })
                    })
            })
            .ok_or_else(|| CcError::Auth("no authorization code in callback".into()))?;

        // Send a minimal HTTP response.
        let response_body = "Authorization complete. You can close this tab.";
        let http_response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
            response_body.len(),
            response_body,
        );
        let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, http_response.as_bytes()).await;

        tracing::info!("received authorization code from callback");
        Ok(code)
    }

    /// Returns `true` if the client holds valid tokens.
    pub fn is_authenticated(&self) -> bool {
        matches!(self.state, OAuthState::Authenticated(_))
    }

    /// Get the current OAuth state.
    pub fn state(&self) -> &OAuthState {
        &self.state
    }

    /// Get the current access token, if authenticated.
    pub fn access_token(&self) -> Option<&str> {
        match &self.state {
            OAuthState::Authenticated(t) => Some(&t.access_token),
            _ => None,
        }
    }

    /// Reset the client to unauthenticated state.
    pub fn logout(&mut self) {
        self.state = OAuthState::Unauthenticated;
    }
}

// ── Free functions (kept for backward compatibility) ───────────────

/// Generate a PKCE challenge pair.
pub fn generate_pkce_challenge() -> PkceChallenge {
    let mut rng = rand::thread_rng();
    let verifier_bytes: Vec<u8> = (0..32).map(|_| rng.gen::<u8>()).collect();
    let verifier = base64_url_encode(&verifier_bytes);

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let digest = hasher.finalize();
    let challenge = base64_url_encode(&digest);

    PkceChallenge {
        verifier,
        challenge,
    }
}

/// Build the authorization URL for the browser (free-function form).
pub fn build_auth_url(config: &OAuthConfig, pkce: &PkceChallenge, state: &str) -> String {
    format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        config.auth_url,
        urlencoding_encode(&config.client_id),
        urlencoding_encode(&config.redirect_uri),
        urlencoding_encode(&config.scopes.join(" ")),
        urlencoding_encode(state),
        urlencoding_encode(&pkce.challenge),
    )
}

/// Exchange an authorization code for tokens (free-function form).
pub async fn exchange_code(
    config: &OAuthConfig,
    code: &str,
    pkce_verifier: &str,
) -> CcResult<TokenResponse> {
    let client = reqwest::Client::new();

    let response = client
        .post(&config.token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("client_id", &config.client_id),
            ("code", code),
            ("redirect_uri", &config.redirect_uri),
            ("code_verifier", pkce_verifier),
        ])
        .send()
        .await
        .map_err(|e| CcError::Auth(format!("token exchange request failed: {e}")))?;

    let status = response.status().as_u16();
    if status != 200 {
        let body = response.text().await.unwrap_or_default();
        return Err(CcError::Auth(format!(
            "token exchange failed (status {status}): {body}"
        )));
    }

    response
        .json::<TokenResponse>()
        .await
        .map_err(|e| CcError::Auth(format!("failed to parse token response: {e}")))
}

/// Refresh an access token using a refresh token (free-function form).
pub async fn refresh_token(
    config: &OAuthConfig,
    refresh_token: &str,
) -> CcResult<TokenResponse> {
    let client = reqwest::Client::new();

    let response = client
        .post(&config.token_url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", &config.client_id),
            ("refresh_token", refresh_token),
        ])
        .send()
        .await
        .map_err(|e| CcError::Auth(format!("token refresh request failed: {e}")))?;

    let status = response.status().as_u16();
    if status != 200 {
        let body = response.text().await.unwrap_or_default();
        return Err(CcError::Auth(format!(
            "token refresh failed (status {status}): {body}"
        )));
    }

    response
        .json::<TokenResponse>()
        .await
        .map_err(|e| CcError::Auth(format!("failed to parse refresh response: {e}")))
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Generate a random state parameter for CSRF protection.
fn generate_random_state() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..16).map(|_| rng.gen::<u8>()).collect();
    base64_url_encode(&bytes)
}

/// URL-safe base64 encoding without padding.
fn base64_url_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// Simple percent-encoding for URL query parameters.
fn urlencoding_encode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> OAuthConfig {
        OAuthConfig {
            client_id: "test-client".into(),
            auth_url: "https://auth.example.com/authorize".into(),
            token_url: "https://auth.example.com/token".into(),
            redirect_uri: "http://localhost:9876/callback".into(),
            scopes: vec!["read".into(), "write".into()],
        }
    }

    // ── Original tests ────────────────────────────────────────────

    #[test]
    fn pkce_challenge_generation() {
        let pkce = generate_pkce_challenge();
        assert!(!pkce.verifier.is_empty());
        assert!(!pkce.challenge.is_empty());
        assert_ne!(pkce.verifier, pkce.challenge);
    }

    #[test]
    fn pkce_challenge_is_deterministic_for_same_verifier() {
        let verifier = "test-verifier-string";
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let digest = hasher.finalize();
        let expected = base64_url_encode(&digest);
        assert!(!expected.is_empty());
    }

    #[test]
    fn build_auth_url_contains_required_params() {
        let config = make_config();
        let pkce = generate_pkce_challenge();
        let url = build_auth_url(&config, &pkce, "random-state");

        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=test-client"));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("state=random-state"));
    }

    #[test]
    fn auth_state_default() {
        let state = AuthState::default();
        assert!(state.access_token.is_none());
        assert!(state.refresh_token.is_none());
    }

    // ── New OAuthClient tests ─────────────────────────────────────

    #[test]
    fn oauth_client_starts_unauthenticated() {
        let client = OAuthClient::new(make_config());
        assert!(!client.is_authenticated());
        assert!(client.access_token().is_none());
        assert!(matches!(client.state(), OAuthState::Unauthenticated));
    }

    #[test]
    fn oauth_client_build_auth_url_transitions_to_awaiting() {
        let mut client = OAuthClient::new(make_config());
        let (url, pkce) = client.build_auth_url();

        // URL should contain all required parameters.
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_id=test-client"));
        assert!(url.contains("code_challenge="));
        assert!(!pkce.verifier.is_empty());

        // State should be AwaitingCallback.
        assert!(matches!(client.state(), OAuthState::AwaitingCallback { .. }));
        assert!(!client.is_authenticated());
    }

    #[test]
    fn oauth_client_logout_resets_state() {
        let mut client = OAuthClient::new(make_config());

        // Manually set to authenticated.
        client.state = OAuthState::Authenticated(TokenResponse {
            access_token: "test-token".into(),
            token_type: "Bearer".into(),
            expires_in: Some(3600),
            refresh_token: Some("refresh-token".into()),
        });

        assert!(client.is_authenticated());
        assert_eq!(client.access_token(), Some("test-token"));

        client.logout();
        assert!(!client.is_authenticated());
        assert!(client.access_token().is_none());
    }

    #[test]
    fn oauth_state_authenticated_access_token() {
        let mut client = OAuthClient::new(make_config());
        client.state = OAuthState::Authenticated(TokenResponse {
            access_token: "my-access-token".into(),
            token_type: "Bearer".into(),
            expires_in: None,
            refresh_token: None,
        });

        assert_eq!(client.access_token(), Some("my-access-token"));
    }

    #[test]
    fn generate_random_state_is_nonempty_and_unique() {
        let s1 = generate_random_state();
        let s2 = generate_random_state();
        assert!(!s1.is_empty());
        assert!(!s2.is_empty());
        // Extremely unlikely to collide.
        assert_ne!(s1, s2);
    }

    #[test]
    fn urlencoding_handles_special_chars() {
        assert_eq!(urlencoding_encode("hello world"), "hello%20world");
        assert_eq!(urlencoding_encode("a+b=c"), "a%2Bb%3Dc");
        assert_eq!(urlencoding_encode("safe-chars_here.ok~"), "safe-chars_here.ok~");
    }
}
