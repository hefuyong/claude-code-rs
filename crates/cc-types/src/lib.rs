//! Core types for the Claude Code RS project.
//!
//! This crate defines the shared data structures used across
//! the entire application, including message types, tool definitions,
//! configuration structures, and common identifiers.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Unique identifier for a conversation session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    /// Create a new random session ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique identifier for a single message.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub Uuid);

impl MessageId {
    /// Create a new random message ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

/// The role of a participant in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// The human user.
    User,
    /// The AI assistant.
    Assistant,
    /// System-level instructions.
    System,
}

/// A block of content within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Plain text content.
    Text { text: String },
    /// A tool use request from the assistant.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// The result of a tool invocation.
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: bool,
    },
}

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier.
    pub id: MessageId,
    /// Role of the message sender.
    pub role: Role,
    /// The content blocks that make up this message.
    pub content: Vec<ContentBlock>,
    /// Timestamp when the message was created.
    pub timestamp: DateTime<Utc>,
}

impl Message {
    /// Create a new user text message.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::User,
            content: vec![ContentBlock::Text { text: text.into() }],
            timestamp: Utc::now(),
        }
    }

    /// Create a new assistant text message.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            id: MessageId::new(),
            role: Role::Assistant,
            content: vec![ContentBlock::Text { text: text.into() }],
            timestamp: Utc::now(),
        }
    }
}

/// Definition of a tool that the assistant can use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// The name of the tool.
    pub name: String,
    /// A description of what the tool does.
    pub description: String,
    /// JSON Schema describing the tool's input parameters.
    pub input_schema: serde_json::Value,
}

/// Token usage information from an API response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Number of input tokens consumed.
    pub input_tokens: u64,
    /// Number of output tokens generated.
    pub output_tokens: u64,
    /// Number of tokens read from cache, if any.
    pub cache_read_tokens: Option<u64>,
    /// Number of tokens written to cache, if any.
    pub cache_creation_tokens: Option<u64>,
}

/// The model to use for API requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelId(pub String);

impl ModelId {
    /// The default Claude model.
    pub fn default_model() -> Self {
        Self("claude-sonnet-4-20250514".to_string())
    }
}

impl Default for ModelId {
    fn default() -> Self {
        Self::default_model()
    }
}

impl std::fmt::Display for ModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
