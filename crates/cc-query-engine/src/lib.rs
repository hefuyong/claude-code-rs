//! Query engine that ties together the query loop, session management,
//! and cost tracking into a single managed unit.

use cc_error::CcResult;
use cc_session::SessionManager;
use cc_types::SessionId;
use serde::{Deserialize, Serialize};

/// Configuration for creating a [`QueryEngine`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEngineConfig {
    /// The model to use.
    pub model: String,
    /// Maximum tokens per turn.
    pub max_tokens: u32,
    /// Maximum agentic turns.
    pub max_turns: u32,
    /// Whether to auto-save the session after each turn.
    pub auto_save: bool,
}

impl Default for QueryEngineConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 16384,
            max_turns: 20,
            auto_save: true,
        }
    }
}

/// The query engine manages a session and its associated query loop.
pub struct QueryEngine {
    /// The session ID for this engine instance.
    session_id: SessionId,
    /// Configuration.
    config: QueryEngineConfig,
    /// The session manager for persistence.
    session_manager: Option<SessionManager>,
}

impl QueryEngine {
    /// Create a new query engine.
    pub fn new(config: QueryEngineConfig) -> CcResult<Self> {
        let session_id = SessionId::new();

        tracing::info!(
            session_id = %session_id.0,
            model = %config.model,
            "query engine created"
        );

        Ok(Self {
            session_id,
            config,
            session_manager: None,
        })
    }

    /// Create a query engine with session persistence.
    pub fn with_session_manager(
        config: QueryEngineConfig,
        session_manager: SessionManager,
    ) -> CcResult<Self> {
        let session_id = SessionId::new();

        tracing::info!(
            session_id = %session_id.0,
            model = %config.model,
            "query engine created with session persistence"
        );

        Ok(Self {
            session_id,
            config,
            session_manager: Some(session_manager),
        })
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Get the engine configuration.
    pub fn config(&self) -> &QueryEngineConfig {
        &self.config
    }

    /// Whether session persistence is enabled.
    pub fn has_persistence(&self) -> bool {
        self.session_manager.is_some()
    }

    /// Get the session manager, if available.
    pub fn session_manager(&self) -> Option<&SessionManager> {
        self.session_manager.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_query_engine() {
        let config = QueryEngineConfig::default();
        let engine = QueryEngine::new(config).unwrap();
        assert!(!engine.has_persistence());
        assert_eq!(engine.config().model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn session_id_is_unique() {
        let e1 = QueryEngine::new(QueryEngineConfig::default()).unwrap();
        let e2 = QueryEngine::new(QueryEngineConfig::default()).unwrap();
        assert_ne!(e1.session_id().0, e2.session_id().0);
    }
}
