//! Session management for Claude Code RS.
//!
//! Persists conversation sessions to disk under `~/.claude/sessions/`
//! so they can be resumed across process restarts.

use cc_api::types::ApiMessage;
use cc_error::{CcError, CcResult};
use cc_types::SessionId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The full data for a persisted session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    /// Unique identifier for this session.
    pub id: SessionId,
    /// The conversation messages in API format.
    pub messages: Vec<ApiMessage>,
    /// The model used for this session.
    pub model: String,
    /// When the session was first created.
    pub created_at: DateTime<Utc>,
    /// When the session was last updated.
    pub updated_at: DateTime<Utc>,
    /// Cumulative cost in USD for this session.
    pub total_cost_usd: f64,
}

impl SessionData {
    /// Create a new empty session.
    pub fn new(model: String) -> Self {
        let now = Utc::now();
        Self {
            id: SessionId::new(),
            messages: Vec::new(),
            model,
            created_at: now,
            updated_at: now,
            total_cost_usd: 0.0,
        }
    }
}

/// A lightweight summary of a session for listing purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// The session's unique identifier.
    pub id: SessionId,
    /// When it was created.
    pub created_at: DateTime<Utc>,
    /// How many messages are in the session.
    pub message_count: usize,
    /// Which model was used.
    pub model: String,
}

/// Manages reading and writing sessions to disk.
pub struct SessionManager {
    /// The directory where session files are stored.
    sessions_dir: PathBuf,
}

impl SessionManager {
    /// Create a new session manager.
    ///
    /// Ensures the `~/.claude/sessions/` directory exists.
    pub fn new() -> CcResult<Self> {
        let home = dirs::home_dir().ok_or_else(|| {
            CcError::Config("could not determine home directory".to_string())
        })?;
        let sessions_dir = home.join(".claude").join("sessions");

        // Create the directory tree if it doesn't exist.
        std::fs::create_dir_all(&sessions_dir).map_err(|e| {
            CcError::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "failed to create sessions directory {}: {}",
                    sessions_dir.display(),
                    e
                ),
            ))
        })?;

        tracing::debug!(dir = %sessions_dir.display(), "session manager initialized");

        Ok(Self { sessions_dir })
    }

    /// Create a session manager with a custom directory (useful for testing).
    pub fn with_dir(sessions_dir: PathBuf) -> CcResult<Self> {
        std::fs::create_dir_all(&sessions_dir)?;
        Ok(Self { sessions_dir })
    }

    /// Build the file path for a given session ID.
    fn session_path(&self, id: &SessionId) -> PathBuf {
        self.sessions_dir.join(format!("{}.json", id.0))
    }

    /// Save a session to disk.
    pub async fn save(&self, session: &SessionData) -> CcResult<()> {
        let path = self.session_path(&session.id);
        let json = serde_json::to_string_pretty(session)
            .map_err(|e| CcError::Serialization(e.to_string()))?;

        tokio::fs::write(&path, json).await.map_err(|e| {
            CcError::Session {
                session_id: Some(session.id.clone()),
                message: format!("failed to write session file: {}", e),
            }
        })?;

        tracing::debug!(
            session_id = %session.id.0,
            path = %path.display(),
            "session saved"
        );
        Ok(())
    }

    /// Load a session from disk.
    pub async fn load(&self, id: &SessionId) -> CcResult<SessionData> {
        let path = self.session_path(id);

        let json = tokio::fs::read_to_string(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CcError::NotFound(format!("session {} not found", id.0))
            } else {
                CcError::Session {
                    session_id: Some(id.clone()),
                    message: format!("failed to read session file: {}", e),
                }
            }
        })?;

        let session: SessionData = serde_json::from_str(&json)
            .map_err(|e| CcError::Serialization(format!("invalid session file: {}", e)))?;

        tracing::debug!(session_id = %id.0, "session loaded");
        Ok(session)
    }

    /// List all saved sessions, sorted by creation time (most recent first).
    pub async fn list(&self) -> CcResult<Vec<SessionSummary>> {
        let mut summaries = Vec::new();

        let mut read_dir = tokio::fs::read_dir(&self.sessions_dir).await.map_err(|e| {
            CcError::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "failed to read sessions directory {}: {}",
                    self.sessions_dir.display(),
                    e
                ),
            ))
        })?;

        while let Some(entry) = read_dir.next_entry().await.map_err(CcError::Io)? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            match tokio::fs::read_to_string(&path).await {
                Ok(json) => match serde_json::from_str::<SessionData>(&json) {
                    Ok(session) => {
                        summaries.push(SessionSummary {
                            id: session.id,
                            created_at: session.created_at,
                            message_count: session.messages.len(),
                            model: session.model,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "skipping invalid session file"
                        );
                    }
                },
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "could not read session file"
                    );
                }
            }
        }

        // Sort by creation time, newest first.
        summaries.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        tracing::debug!(count = summaries.len(), "listed sessions");
        Ok(summaries)
    }

    /// Delete a session from disk.
    pub async fn delete(&self, id: &SessionId) -> CcResult<()> {
        let path = self.session_path(id);

        tokio::fs::remove_file(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                CcError::NotFound(format!("session {} not found", id.0))
            } else {
                CcError::Session {
                    session_id: Some(id.clone()),
                    message: format!("failed to delete session file: {}", e),
                }
            }
        })?;

        tracing::debug!(session_id = %id.0, "session deleted");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_api::types::{ApiContent, ApiMessage};

    fn make_session() -> SessionData {
        let mut session = SessionData::new("claude-sonnet-4-20250514".to_string());
        session.messages.push(ApiMessage {
            role: "user".to_string(),
            content: ApiContent::Text("Hello".to_string()),
        });
        session.messages.push(ApiMessage {
            role: "assistant".to_string(),
            content: ApiContent::Text("Hi there!".to_string()),
        });
        session
    }

    #[tokio::test]
    async fn save_and_load_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = SessionManager::with_dir(tmp.path().to_path_buf()).unwrap();

        let session = make_session();
        let id = session.id.clone();

        mgr.save(&session).await.unwrap();
        let loaded = mgr.load(&id).await.unwrap();

        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.model, session.model);
        assert_eq!(loaded.messages.len(), 2);
    }

    #[tokio::test]
    async fn list_sessions() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = SessionManager::with_dir(tmp.path().to_path_buf()).unwrap();

        let s1 = make_session();
        let s2 = make_session();
        mgr.save(&s1).await.unwrap();
        mgr.save(&s2).await.unwrap();

        let summaries = mgr.list().await.unwrap();
        assert_eq!(summaries.len(), 2);
    }

    #[tokio::test]
    async fn delete_session() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = SessionManager::with_dir(tmp.path().to_path_buf()).unwrap();

        let session = make_session();
        let id = session.id.clone();

        mgr.save(&session).await.unwrap();
        mgr.delete(&id).await.unwrap();

        let result = mgr.load(&id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn load_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let mgr = SessionManager::with_dir(tmp.path().to_path_buf()).unwrap();

        let id = SessionId::new();
        let result = mgr.load(&id).await;
        assert!(result.is_err());
    }
}
