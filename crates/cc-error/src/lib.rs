//! Error types for the Claude Code RS project.
//!
//! Provides a unified error hierarchy used across all crates.
//! Each subsystem has its own error variant, and they all compose
//! into the top-level [`CcError`] enum.

use cc_types::SessionId;
use thiserror::Error;

/// Top-level error type for the Claude Code RS application.
#[derive(Debug, Error)]
pub enum CcError {
    /// An error originating from the API layer.
    #[error("API error: {message}")]
    Api {
        message: String,
        status_code: Option<u16>,
    },

    /// An error in configuration loading or validation.
    #[error("Configuration error: {0}")]
    Config(String),

    /// A session-related error.
    #[error("Session error for {session_id:?}: {message}")]
    Session {
        session_id: Option<SessionId>,
        message: String,
    },

    /// A tool execution error.
    #[error("Tool error in '{tool_name}': {message}")]
    Tool { tool_name: String, message: String },

    /// A permission denied error.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// A resource was not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// An I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// An internal error that should not normally occur.
    #[error("Internal error: {0}")]
    Internal(String),

    /// Operation was cancelled (e.g., by user interrupt).
    #[error("Operation cancelled: {0}")]
    Cancelled(String),

    /// Rate limit exceeded.
    #[error("Rate limited: retry after {retry_after_secs:?}s")]
    RateLimited { retry_after_secs: Option<u64> },

    /// Authentication failure.
    #[error("Authentication error: {0}")]
    Auth(String),
}

/// A convenience type alias for results using [`CcError`].
pub type CcResult<T> = Result<T, CcError>;

impl CcError {
    /// Returns true if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CcError::RateLimited { .. }
                | CcError::Api {
                    status_code: Some(429 | 500 | 502 | 503),
                    ..
                }
        )
    }

    /// Returns true if this error represents a user cancellation.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, CcError::Cancelled(_))
    }

    /// Create an API error with a status code.
    pub fn api(message: impl Into<String>, status_code: u16) -> Self {
        CcError::Api {
            message: message.into(),
            status_code: Some(status_code),
        }
    }

    /// Create a tool error.
    pub fn tool(name: impl Into<String>, message: impl Into<String>) -> Self {
        CcError::Tool {
            tool_name: name.into(),
            message: message.into(),
        }
    }
}
