//! Message handling for Claude Code RS.
//!
//! Provides conversation history management, message construction,
//! and serialization for API communication.

use cc_error::CcResult;
use cc_types::{ContentBlock, Message, MessageId, Role, SessionId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A full conversation containing an ordered list of messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    /// The session this conversation belongs to.
    pub session_id: SessionId,
    /// The ordered list of messages.
    pub messages: Vec<Message>,
    /// When the conversation was created.
    pub created_at: DateTime<Utc>,
    /// When the conversation was last updated.
    pub updated_at: DateTime<Utc>,
}

impl Conversation {
    /// Create a new empty conversation.
    pub fn new(session_id: SessionId) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a message to the conversation.
    pub fn push(&mut self, message: Message) {
        self.updated_at = Utc::now();
        self.messages.push(message);
    }

    /// Get the number of messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if the conversation is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get the last message, if any.
    pub fn last(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// Extract all text content from messages with the given role.
    pub fn text_for_role(&self, role: Role) -> Vec<String> {
        self.messages
            .iter()
            .filter(|m| m.role == role)
            .flat_map(|m| {
                m.content.iter().filter_map(|block| match block {
                    ContentBlock::Text { text } => Some(text.clone()),
                    _ => None,
                })
            })
            .collect()
    }
}

/// Builder for constructing messages with multiple content blocks.
pub struct MessageBuilder {
    role: Role,
    content: Vec<ContentBlock>,
}

impl MessageBuilder {
    /// Start building a user message.
    pub fn user() -> Self {
        Self {
            role: Role::User,
            content: Vec::new(),
        }
    }

    /// Start building an assistant message.
    pub fn assistant() -> Self {
        Self {
            role: Role::Assistant,
            content: Vec::new(),
        }
    }

    /// Add a text block.
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.content.push(ContentBlock::Text { text: text.into() });
        self
    }

    /// Add a tool use block.
    pub fn tool_use(
        mut self,
        id: impl Into<String>,
        name: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        self.content.push(ContentBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        });
        self
    }

    /// Add a tool result block.
    pub fn tool_result(
        mut self,
        tool_use_id: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        self.content.push(ContentBlock::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error,
        });
        self
    }

    /// Build the final message.
    pub fn build(self) -> Message {
        Message {
            id: MessageId::new(),
            role: self.role,
            content: self.content,
            timestamp: Utc::now(),
        }
    }
}

/// Serialize a conversation to JSON for API requests.
pub fn serialize_for_api(conversation: &Conversation) -> CcResult<serde_json::Value> {
    let messages: Vec<serde_json::Value> = conversation
        .messages
        .iter()
        .map(|msg| {
            serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            })
        })
        .collect();

    Ok(serde_json::Value::Array(messages))
}
