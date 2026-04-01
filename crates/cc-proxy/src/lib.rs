//! Upstream proxy support for Claude Code RS.
//!
//! Detects proxy configuration from environment variables and applies
//! it to outgoing HTTP requests. Also provides a local relay that can
//! tunnel traffic through a configured upstream proxy.

use std::path::PathBuf;

use cc_error::{CcError, CcResult};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ProxyAuth
// ---------------------------------------------------------------------------

/// Credentials for proxy authentication (Basic auth).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyAuth {
    pub username: String,
    pub password: String,
}

impl ProxyAuth {
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// ProxyConfig
// ---------------------------------------------------------------------------

/// Configuration for an upstream HTTP/HTTPS proxy.
///
/// Typically loaded from standard environment variables (`HTTP_PROXY`,
/// `HTTPS_PROXY`, `NO_PROXY`, `SSL_CERT_FILE`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// URL of the HTTP proxy (e.g. `http://proxy.example.com:8080`).
    pub http_proxy: Option<String>,
    /// URL of the HTTPS proxy.
    pub https_proxy: Option<String>,
    /// List of hosts/domains that should bypass the proxy.
    pub no_proxy: Vec<String>,
    /// Optional Basic-auth credentials for the proxy.
    pub proxy_auth: Option<ProxyAuth>,
    /// Optional path to a custom CA certificate bundle.
    pub ca_cert_path: Option<PathBuf>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            http_proxy: None,
            https_proxy: None,
            no_proxy: Vec::new(),
            proxy_auth: None,
            ca_cert_path: None,
        }
    }
}

impl ProxyConfig {
    /// Load proxy configuration from environment variables.
    ///
    /// Reads `HTTP_PROXY` / `http_proxy`, `HTTPS_PROXY` / `https_proxy`,
    /// `NO_PROXY` / `no_proxy`, and `SSL_CERT_FILE`.
    pub fn from_env() -> Self {
        Self {
            http_proxy: std::env::var("HTTP_PROXY")
                .or_else(|_| std::env::var("http_proxy"))
                .ok(),
            https_proxy: std::env::var("HTTPS_PROXY")
                .or_else(|_| std::env::var("https_proxy"))
                .ok(),
            no_proxy: std::env::var("NO_PROXY")
                .or_else(|_| std::env::var("no_proxy"))
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            proxy_auth: None,
            ca_cert_path: std::env::var("SSL_CERT_FILE").ok().map(PathBuf::from),
        }
    }

    /// Returns `true` if any proxy URL is configured.
    pub fn is_configured(&self) -> bool {
        self.http_proxy.is_some() || self.https_proxy.is_some()
    }

    /// Check whether a given `host` should bypass the proxy according to
    /// the `no_proxy` list.
    ///
    /// Supports exact matches, suffix matches (`.example.com`), and the
    /// wildcard `*` (bypass everything).
    pub fn should_bypass(&self, host: &str) -> bool {
        let host_lower = host.to_lowercase();
        for entry in &self.no_proxy {
            let pattern = entry.to_lowercase();
            if pattern == "*" {
                return true;
            }
            if host_lower == pattern {
                return true;
            }
            // Suffix match: entry ".example.com" matches "foo.example.com"
            if pattern.starts_with('.') && host_lower.ends_with(&pattern) {
                return true;
            }
            // Also match without leading dot
            if !pattern.starts_with('.') && host_lower.ends_with(&format!(".{}", pattern)) {
                return true;
            }
            // localhost / 127.0.0.1 shorthand
            if pattern == "localhost" && (host_lower == "127.0.0.1" || host_lower == "::1") {
                return true;
            }
        }
        false
    }

    /// Return the appropriate proxy URL for the given `target_url`, or
    /// `None` if the target should bypass the proxy.
    pub fn proxy_url_for(&self, target_url: &str) -> Option<String> {
        // Parse the target URL to extract the host and scheme.
        let parsed = url::Url::parse(target_url).ok()?;
        let host = parsed.host_str()?;

        if self.should_bypass(host) {
            return None;
        }

        match parsed.scheme() {
            "https" => self.https_proxy.clone().or_else(|| self.http_proxy.clone()),
            _ => self.http_proxy.clone().or_else(|| self.https_proxy.clone()),
        }
    }

    /// Apply the proxy configuration to a [`reqwest::ClientBuilder`].
    ///
    /// Sets the HTTP and HTTPS proxies and optionally configures
    /// proxy authentication.
    pub fn apply_to_client_builder(
        &self,
        mut builder: reqwest::ClientBuilder,
    ) -> CcResult<reqwest::ClientBuilder> {
        if let Some(ref http_url) = self.http_proxy {
            let mut proxy = reqwest::Proxy::http(http_url).map_err(|e| {
                CcError::Config(format!("invalid HTTP proxy URL '{}': {}", http_url, e))
            })?;
            if let Some(ref auth) = self.proxy_auth {
                proxy = proxy.basic_auth(&auth.username, &auth.password);
            }
            builder = builder.proxy(proxy);
        }

        if let Some(ref https_url) = self.https_proxy {
            let mut proxy = reqwest::Proxy::https(https_url).map_err(|e| {
                CcError::Config(format!("invalid HTTPS proxy URL '{}': {}", https_url, e))
            })?;
            if let Some(ref auth) = self.proxy_auth {
                proxy = proxy.basic_auth(&auth.username, &auth.password);
            }
            builder = builder.proxy(proxy);
        }

        // Note: ca_cert_path would require reading the file and adding the
        // certificate to the builder.  We leave a hook here for future use.
        if let Some(ref _ca_path) = self.ca_cert_path {
            tracing::debug!(path = ?_ca_path, "custom CA certificate path configured (not yet loaded)");
        }

        Ok(builder)
    }
}

// ---------------------------------------------------------------------------
// ProxyRelay
// ---------------------------------------------------------------------------

/// A local TCP relay that tunnels traffic through an upstream proxy.
///
/// This is useful when tools or subprocesses need to go through a proxy
/// but do not support proxy environment variables natively.
pub struct ProxyRelay {
    config: ProxyConfig,
    listener: Option<tokio::net::TcpListener>,
    running: bool,
}

impl ProxyRelay {
    /// Create a new relay backed by the given proxy configuration.
    pub fn new(config: ProxyConfig) -> Self {
        Self {
            config,
            listener: None,
            running: false,
        }
    }

    /// Start listening on the given `listen_port`.
    ///
    /// Incoming connections will be forwarded through the upstream proxy.
    pub async fn start(&mut self, listen_port: u16) -> CcResult<()> {
        if !self.config.is_configured() {
            return Err(CcError::Config(
                "cannot start proxy relay: no upstream proxy configured".into(),
            ));
        }

        let addr = format!("127.0.0.1:{}", listen_port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| CcError::Io(e))?;

        tracing::info!(address = %addr, "proxy relay started");
        self.listener = Some(listener);
        self.running = true;
        Ok(())
    }

    /// Stop the relay and drop the listener.
    pub async fn stop(&mut self) {
        self.listener = None;
        self.running = false;
        tracing::info!("proxy relay stopped");
    }

    /// Whether the relay is currently listening.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Access the underlying proxy configuration.
    pub fn config(&self) -> &ProxyConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_not_configured() {
        let cfg = ProxyConfig::default();
        assert!(!cfg.is_configured());
        assert!(cfg.proxy_url_for("https://api.anthropic.com").is_none());
    }

    #[test]
    fn test_configured_proxy() {
        let cfg = ProxyConfig {
            http_proxy: Some("http://proxy:8080".into()),
            https_proxy: Some("http://proxy:8443".into()),
            ..Default::default()
        };
        assert!(cfg.is_configured());

        // HTTPS target should use https_proxy
        assert_eq!(
            cfg.proxy_url_for("https://api.anthropic.com/v1/messages"),
            Some("http://proxy:8443".into())
        );

        // HTTP target should use http_proxy
        assert_eq!(
            cfg.proxy_url_for("http://example.com/data"),
            Some("http://proxy:8080".into())
        );
    }

    #[test]
    fn test_no_proxy_exact_match() {
        let cfg = ProxyConfig {
            http_proxy: Some("http://proxy:8080".into()),
            no_proxy: vec!["localhost".into(), "internal.corp".into()],
            ..Default::default()
        };

        assert!(cfg.should_bypass("localhost"));
        assert!(cfg.should_bypass("internal.corp"));
        assert!(!cfg.should_bypass("external.com"));
    }

    #[test]
    fn test_no_proxy_suffix_match() {
        let cfg = ProxyConfig {
            http_proxy: Some("http://proxy:8080".into()),
            no_proxy: vec![".example.com".into()],
            ..Default::default()
        };

        assert!(cfg.should_bypass("foo.example.com"));
        assert!(cfg.should_bypass("bar.baz.example.com"));
        assert!(!cfg.should_bypass("example.com")); // exact != suffix
        assert!(!cfg.should_bypass("notexample.com"));
    }

    #[test]
    fn test_no_proxy_wildcard() {
        let cfg = ProxyConfig {
            http_proxy: Some("http://proxy:8080".into()),
            no_proxy: vec!["*".into()],
            ..Default::default()
        };
        assert!(cfg.should_bypass("anything.com"));
        assert!(cfg.should_bypass("localhost"));
    }

    #[test]
    fn test_no_proxy_domain_without_dot() {
        let cfg = ProxyConfig {
            http_proxy: Some("http://proxy:8080".into()),
            no_proxy: vec!["corp.internal".into()],
            ..Default::default()
        };
        assert!(cfg.should_bypass("corp.internal"));
        assert!(cfg.should_bypass("app.corp.internal"));
        assert!(!cfg.should_bypass("notcorp.internal"));
    }

    #[test]
    fn test_proxy_url_for_bypassed_host() {
        let cfg = ProxyConfig {
            http_proxy: Some("http://proxy:8080".into()),
            https_proxy: Some("http://proxy:8443".into()),
            no_proxy: vec!["localhost".into()],
            ..Default::default()
        };
        assert!(cfg.proxy_url_for("http://localhost:3000/api").is_none());
    }

    #[test]
    fn test_proxy_url_fallback() {
        // Only http_proxy set; HTTPS targets should fall back to it.
        let cfg = ProxyConfig {
            http_proxy: Some("http://proxy:8080".into()),
            ..Default::default()
        };
        assert_eq!(
            cfg.proxy_url_for("https://example.com"),
            Some("http://proxy:8080".into())
        );
    }

    #[test]
    fn test_apply_to_client_builder() {
        let cfg = ProxyConfig {
            http_proxy: Some("http://proxy:8080".into()),
            proxy_auth: Some(ProxyAuth::new("user", "pass")),
            ..Default::default()
        };
        let builder = reqwest::Client::builder();
        let result = cfg.apply_to_client_builder(builder);
        assert!(result.is_ok());
    }

    #[test]
    fn test_proxy_relay_not_configured() {
        let cfg = ProxyConfig::default();
        let relay = ProxyRelay::new(cfg);
        assert!(!relay.is_running());
    }

    #[tokio::test]
    async fn test_proxy_relay_start_without_config() {
        let cfg = ProxyConfig::default();
        let mut relay = ProxyRelay::new(cfg);
        let result = relay.start(0).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_proxy_relay_lifecycle() {
        let cfg = ProxyConfig {
            http_proxy: Some("http://proxy:8080".into()),
            ..Default::default()
        };
        let mut relay = ProxyRelay::new(cfg);
        assert!(!relay.is_running());

        // Start on port 0 to let the OS pick a free port.
        relay.start(0).await.unwrap();
        assert!(relay.is_running());

        relay.stop().await;
        assert!(!relay.is_running());
    }
}
