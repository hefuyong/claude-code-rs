//! MCP Elicitation handling.
//!
//! When an MCP server needs interactive input from the user (e.g.
//! authentication, confirmation, or text input), it issues an
//! *elicitation request*.  This module provides the handler that
//! queues those requests and lets the UI layer resolve them.

use std::collections::HashMap;

use cc_error::{CcError, CcResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ElicitationAction
// ---------------------------------------------------------------------------

/// The kind of input the MCP server is requesting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElicitationAction {
    /// The server wants the user to authenticate via a URL.
    UrlAuth {
        url: String,
    },
    /// The server wants free-form text input.
    TextInput {
        prompt: String,
        placeholder: Option<String>,
    },
    /// The server wants a yes/no confirmation.
    Confirm {
        message: String,
    },
    /// The server wants the user to pick from a list of options.
    Select {
        options: Vec<String>,
    },
}

// ---------------------------------------------------------------------------
// ElicitationRequest
// ---------------------------------------------------------------------------

/// A pending elicitation request from an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElicitationRequest {
    /// Unique request identifier.
    pub id: String,
    /// Name of the MCP server that issued the request.
    pub server_name: String,
    /// What kind of input is needed.
    pub action: ElicitationAction,
    /// Human-readable message to show to the user.
    pub message: String,
    /// When the request was created.
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// ElicitationResponse
// ---------------------------------------------------------------------------

/// The outcome of an elicitation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ElicitationResponse {
    /// The user provided the requested data.
    Completed { data: serde_json::Value },
    /// The user cancelled the request.
    Cancelled,
    /// The request timed out before the user responded.
    TimedOut,
}

// ---------------------------------------------------------------------------
// ElicitationHandler
// ---------------------------------------------------------------------------

/// Manages the lifecycle of MCP elicitation requests.
pub struct ElicitationHandler {
    pending: HashMap<String, ElicitationRequest>,
}

impl ElicitationHandler {
    /// Create a new, empty handler.
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
        }
    }

    /// Submit a new elicitation request and return its ID.
    pub fn request(
        &mut self,
        server: &str,
        action: ElicitationAction,
        message: &str,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let req = ElicitationRequest {
            id: id.clone(),
            server_name: server.to_string(),
            action,
            message: message.to_string(),
            created_at: Utc::now(),
        };
        tracing::debug!(id = %id, server = server, "elicitation request created");
        self.pending.insert(id.clone(), req);
        id
    }

    /// Resolve (complete, cancel, or time-out) a pending request.
    pub fn resolve(&mut self, id: &str, response: ElicitationResponse) -> CcResult<()> {
        if self.pending.remove(id).is_none() {
            return Err(CcError::NotFound(format!(
                "elicitation request '{}' not found",
                id
            )));
        }
        tracing::debug!(id = id, outcome = ?response, "elicitation resolved");
        Ok(())
    }

    /// Return all currently pending requests.
    pub fn pending_requests(&self) -> Vec<&ElicitationRequest> {
        self.pending.values().collect()
    }

    /// Cancel every pending request from a given server.
    pub fn cancel_all(&mut self, server: &str) {
        self.pending.retain(|_, req| req.server_name != server);
        tracing::debug!(server = server, "all elicitation requests cancelled");
    }

    /// Number of pending requests.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

impl Default for ElicitationHandler {
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
    fn test_request_and_resolve() {
        let mut handler = ElicitationHandler::new();

        let id = handler.request(
            "test-server",
            ElicitationAction::Confirm {
                message: "Allow access?".into(),
            },
            "Please confirm",
        );
        assert_eq!(handler.pending_count(), 1);

        handler
            .resolve(
                &id,
                ElicitationResponse::Completed {
                    data: serde_json::json!(true),
                },
            )
            .unwrap();
        assert_eq!(handler.pending_count(), 0);
    }

    #[test]
    fn test_resolve_unknown_id_errors() {
        let mut handler = ElicitationHandler::new();
        let result = handler.resolve("nonexistent", ElicitationResponse::Cancelled);
        assert!(result.is_err());
    }

    #[test]
    fn test_cancel_all_for_server() {
        let mut handler = ElicitationHandler::new();

        handler.request(
            "server-a",
            ElicitationAction::TextInput {
                prompt: "name?".into(),
                placeholder: None,
            },
            "Enter name",
        );
        handler.request(
            "server-a",
            ElicitationAction::Confirm {
                message: "ok?".into(),
            },
            "Confirm",
        );
        handler.request(
            "server-b",
            ElicitationAction::UrlAuth {
                url: "https://auth.example.com".into(),
            },
            "Authenticate",
        );

        assert_eq!(handler.pending_count(), 3);
        handler.cancel_all("server-a");
        assert_eq!(handler.pending_count(), 1);

        // The remaining request should belong to server-b.
        let remaining = handler.pending_requests();
        assert_eq!(remaining[0].server_name, "server-b");
    }

    #[test]
    fn test_pending_requests_returns_all() {
        let mut handler = ElicitationHandler::new();
        handler.request(
            "s1",
            ElicitationAction::Select {
                options: vec!["a".into(), "b".into()],
            },
            "Pick one",
        );
        handler.request(
            "s2",
            ElicitationAction::Confirm {
                message: "sure?".into(),
            },
            "Confirm",
        );
        assert_eq!(handler.pending_requests().len(), 2);
    }

    #[test]
    fn test_elicitation_action_serialization() {
        let action = ElicitationAction::TextInput {
            prompt: "What is your name?".into(),
            placeholder: Some("John Doe".into()),
        };
        let json = serde_json::to_value(&action).unwrap();
        assert!(json.get("TextInput").is_some());
    }
}
