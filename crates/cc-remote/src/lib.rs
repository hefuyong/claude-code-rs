//! Remote session management for Claude Code RS.
//!
//! Allows running Claude Code sessions on remote machines via a
//! WebSocket-based connection to a session server. The manager
//! tracks multiple sessions, each with its own connection state
//! and message channel.

use cc_error::{CcError, CcResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ── Configuration ──────────────────────────────────────────────────────

/// Configuration for connecting to a remote session server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConfig {
    /// The URL of the remote session server (e.g. `wss://host:port/sessions`).
    pub server_url: String,
    /// An optional bearer token for authentication.
    pub auth_token: Option<String>,
    /// Connection timeout in seconds.
    #[serde(default = "default_timeout")]
    pub connect_timeout_secs: u64,
    /// Maximum number of reconnection attempts on disconnect.
    #[serde(default = "default_max_reconnects")]
    pub max_reconnects: u32,
}

fn default_timeout() -> u64 {
    30
}

fn default_max_reconnects() -> u32 {
    5
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            server_url: "wss://localhost:9800/sessions".to_string(),
            auth_token: None,
            connect_timeout_secs: default_timeout(),
            max_reconnects: default_max_reconnects(),
        }
    }
}

// ── Session types ──────────────────────────────────────────────────────

/// The lifecycle status of a remote session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteSessionStatus {
    /// The session is being established.
    Connecting,
    /// The session is live and accepting messages.
    Active,
    /// The session is paused but can be resumed.
    Suspended,
    /// The session has been terminated.
    Disconnected,
}

/// A single remote session with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSession {
    /// Unique session identifier.
    pub id: String,
    /// Current status.
    pub status: RemoteSessionStatus,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session last had activity.
    pub last_activity: DateTime<Utc>,
    /// Number of messages exchanged.
    pub message_count: u64,
}

impl RemoteSession {
    /// Create a new session in `Connecting` state.
    fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            status: RemoteSessionStatus::Connecting,
            created_at: now,
            last_activity: now,
            message_count: 0,
        }
    }
}

// ── Manager ────────────────────────────────────────────────────────────

/// Manages remote Claude Code sessions.
///
/// Each session is tracked by ID and can be independently connected,
/// suspended, and disconnected. Messages are exchanged through the
/// send/receive methods.
pub struct RemoteSessionManager {
    sessions: HashMap<String, RemoteSession>,
    config: RemoteConfig,
}

impl RemoteSessionManager {
    /// Create a new remote session manager with the given configuration.
    pub fn new(config: RemoteConfig) -> Self {
        Self {
            sessions: HashMap::new(),
            config,
        }
    }

    /// Check if remote session support is available.
    ///
    /// Returns `true` when the server URL is configured and non-empty.
    pub fn is_available(&self) -> bool {
        !self.config.server_url.is_empty()
    }

    /// Create a new session and register it in the manager.
    ///
    /// The session starts in `Connecting` state. Call [`connect`] to
    /// establish the actual WebSocket connection.
    pub async fn create_session(&mut self) -> CcResult<String> {
        let session = RemoteSession::new();
        let id = session.id.clone();
        tracing::info!(session_id = %id, server = %self.config.server_url, "creating remote session");
        self.sessions.insert(id.clone(), session);
        Ok(id)
    }

    /// Establish a connection for the given session.
    ///
    /// In this implementation the connection is simulated -- a real
    /// implementation would open a WebSocket using `tokio-tungstenite`
    /// and perform the handshake.
    pub async fn connect(&mut self, session_id: &str) -> CcResult<()> {
        let session = self.sessions.get_mut(session_id).ok_or_else(|| {
            CcError::NotFound(format!("remote session '{session_id}' not found"))
        })?;

        if session.status == RemoteSessionStatus::Active {
            return Ok(()); // already connected
        }

        tracing::info!(session_id, "connecting to remote session");
        session.status = RemoteSessionStatus::Active;
        session.last_activity = Utc::now();
        Ok(())
    }

    /// Disconnect a session, moving it to `Disconnected` state.
    pub async fn disconnect(&mut self, session_id: &str) -> CcResult<()> {
        let session = self.sessions.get_mut(session_id).ok_or_else(|| {
            CcError::NotFound(format!("remote session '{session_id}' not found"))
        })?;

        tracing::info!(session_id, "disconnecting remote session");
        session.status = RemoteSessionStatus::Disconnected;
        session.last_activity = Utc::now();
        Ok(())
    }

    /// Suspend a session, moving it to `Suspended` state.
    pub async fn suspend(&mut self, session_id: &str) -> CcResult<()> {
        let session = self.sessions.get_mut(session_id).ok_or_else(|| {
            CcError::NotFound(format!("remote session '{session_id}' not found"))
        })?;

        if session.status != RemoteSessionStatus::Active {
            return Err(CcError::Internal(format!(
                "cannot suspend session in {:?} state",
                session.status,
            )));
        }

        tracing::info!(session_id, "suspending remote session");
        session.status = RemoteSessionStatus::Suspended;
        session.last_activity = Utc::now();
        Ok(())
    }

    /// Send a message to the given session.
    ///
    /// The session must be in `Active` state.
    pub async fn send_message(&mut self, session_id: &str, msg: &str) -> CcResult<()> {
        let session = self.sessions.get_mut(session_id).ok_or_else(|| {
            CcError::NotFound(format!("remote session '{session_id}' not found"))
        })?;

        if session.status != RemoteSessionStatus::Active {
            return Err(CcError::Internal(format!(
                "cannot send message to session in {:?} state",
                session.status,
            )));
        }

        tracing::debug!(session_id, bytes = msg.len(), "sending message");
        session.message_count += 1;
        session.last_activity = Utc::now();
        Ok(())
    }

    /// Receive the next message from the given session.
    ///
    /// Returns `None` if no message is currently available.
    pub async fn receive_message(&self, session_id: &str) -> CcResult<Option<String>> {
        let session = self.sessions.get(session_id).ok_or_else(|| {
            CcError::NotFound(format!("remote session '{session_id}' not found"))
        })?;

        if session.status != RemoteSessionStatus::Active {
            return Err(CcError::Internal(format!(
                "cannot receive from session in {:?} state",
                session.status,
            )));
        }

        // In a real implementation this would read from the WebSocket.
        Ok(None)
    }

    /// List all managed sessions.
    pub fn list_sessions(&self) -> Vec<&RemoteSession> {
        let mut sessions: Vec<&RemoteSession> = self.sessions.values().collect();
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        sessions
    }

    /// Get a single session by ID.
    pub fn get_session(&self, session_id: &str) -> Option<&RemoteSession> {
        self.sessions.get(session_id)
    }

    /// Remove a disconnected session from the manager.
    pub fn remove_session(&mut self, session_id: &str) -> CcResult<()> {
        let session = self.sessions.get(session_id).ok_or_else(|| {
            CcError::NotFound(format!("remote session '{session_id}' not found"))
        })?;

        if session.status == RemoteSessionStatus::Active {
            return Err(CcError::Internal(
                "cannot remove an active session -- disconnect first".into(),
            ));
        }

        self.sessions.remove(session_id);
        tracing::info!(session_id, "remote session removed");
        Ok(())
    }
}

impl Default for RemoteSessionManager {
    fn default() -> Self {
        Self::new(RemoteConfig::default())
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_manager() -> RemoteSessionManager {
        RemoteSessionManager::new(RemoteConfig {
            server_url: "wss://test-server:9800/sessions".into(),
            auth_token: Some("test-token".into()),
            connect_timeout_secs: 5,
            max_reconnects: 3,
        })
    }

    #[test]
    fn is_available_with_config() {
        let mgr = make_manager();
        assert!(mgr.is_available());

        let empty = RemoteSessionManager::new(RemoteConfig {
            server_url: String::new(),
            ..Default::default()
        });
        assert!(!empty.is_available());
    }

    #[tokio::test]
    async fn create_and_connect_session() {
        let mut mgr = make_manager();

        let id = mgr.create_session().await.unwrap();
        assert!(!id.is_empty());

        // Should be in Connecting state.
        let session = mgr.get_session(&id).unwrap();
        assert_eq!(session.status, RemoteSessionStatus::Connecting);

        // Connect it.
        mgr.connect(&id).await.unwrap();
        let session = mgr.get_session(&id).unwrap();
        assert_eq!(session.status, RemoteSessionStatus::Active);

        // Connecting again when already active is a no-op.
        mgr.connect(&id).await.unwrap();
    }

    #[tokio::test]
    async fn send_requires_active_session() {
        let mut mgr = make_manager();
        let id = mgr.create_session().await.unwrap();

        // Sending before connecting should fail.
        let result = mgr.send_message(&id, "hello").await;
        assert!(result.is_err());

        // After connecting it should succeed.
        mgr.connect(&id).await.unwrap();
        mgr.send_message(&id, "hello").await.unwrap();

        let session = mgr.get_session(&id).unwrap();
        assert_eq!(session.message_count, 1);
    }

    #[tokio::test]
    async fn disconnect_and_remove() {
        let mut mgr = make_manager();
        let id = mgr.create_session().await.unwrap();
        mgr.connect(&id).await.unwrap();

        // Cannot remove while active.
        assert!(mgr.remove_session(&id).is_err());

        // Disconnect, then remove.
        mgr.disconnect(&id).await.unwrap();
        let session = mgr.get_session(&id).unwrap();
        assert_eq!(session.status, RemoteSessionStatus::Disconnected);

        mgr.remove_session(&id).unwrap();
        assert!(mgr.get_session(&id).is_none());
    }

    #[tokio::test]
    async fn suspend_and_reconnect() {
        let mut mgr = make_manager();
        let id = mgr.create_session().await.unwrap();
        mgr.connect(&id).await.unwrap();

        mgr.suspend(&id).await.unwrap();
        let session = mgr.get_session(&id).unwrap();
        assert_eq!(session.status, RemoteSessionStatus::Suspended);

        // Reconnect from suspended state.
        mgr.connect(&id).await.unwrap();
        let session = mgr.get_session(&id).unwrap();
        assert_eq!(session.status, RemoteSessionStatus::Active);
    }

    #[tokio::test]
    async fn list_sessions_sorted() {
        let mut mgr = make_manager();
        let _id1 = mgr.create_session().await.unwrap();
        let _id2 = mgr.create_session().await.unwrap();
        let _id3 = mgr.create_session().await.unwrap();

        let sessions = mgr.list_sessions();
        assert_eq!(sessions.len(), 3);
    }

    #[test]
    fn not_found_errors() {
        let mgr = make_manager();
        assert!(mgr.get_session("nonexistent").is_none());
    }

    #[tokio::test]
    async fn receive_returns_none() {
        let mut mgr = make_manager();
        let id = mgr.create_session().await.unwrap();
        mgr.connect(&id).await.unwrap();

        let msg = mgr.receive_message(&id).await.unwrap();
        assert!(msg.is_none());
    }

    #[test]
    fn remote_config_default() {
        let cfg = RemoteConfig::default();
        assert_eq!(cfg.connect_timeout_secs, 30);
        assert_eq!(cfg.max_reconnects, 5);
        assert!(cfg.auth_token.is_none());
    }

    #[test]
    fn session_status_serialization() {
        let json = serde_json::to_string(&RemoteSessionStatus::Active).unwrap();
        assert_eq!(json, "\"active\"");

        let parsed: RemoteSessionStatus = serde_json::from_str("\"suspended\"").unwrap();
        assert_eq!(parsed, RemoteSessionStatus::Suspended);
    }
}
