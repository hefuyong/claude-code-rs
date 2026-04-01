//! Bridge message types and a simple message queue.
//!
//! Defines the protocol messages exchanged between the bridge client
//! and the claude.ai server over the WebSocket connection.

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use cc_error::{CcError, CcResult};

/// A message exchanged over the bridge connection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BridgeMessage {
    /// User text input to the assistant.
    UserInput {
        text: String,
        session_id: String,
    },
    /// Assistant text output to the user.
    AssistantOutput {
        text: String,
        session_id: String,
    },
    /// The assistant wants to invoke a tool.
    ToolUse {
        tool_name: String,
        input: serde_json::Value,
        session_id: String,
    },
    /// The result of a tool invocation.
    ToolResult {
        output: String,
        is_error: bool,
        session_id: String,
    },
    /// A new session has started.
    SessionStart {
        session_id: String,
    },
    /// A session has ended.
    SessionEnd {
        session_id: String,
    },
    /// Keep-alive heartbeat.
    Heartbeat,
    /// An error notification.
    Error {
        code: String,
        message: String,
    },
}

impl BridgeMessage {
    /// Serialize this message to a JSON string.
    pub fn serialize(&self) -> CcResult<String> {
        serde_json::to_string(self)
            .map_err(|e| CcError::Serialization(format!("Failed to serialize bridge message: {e}")))
    }

    /// Deserialize a bridge message from a JSON string.
    pub fn deserialize(data: &str) -> CcResult<Self> {
        serde_json::from_str(data).map_err(|e| {
            CcError::Serialization(format!("Failed to deserialize bridge message: {e}"))
        })
    }

    /// Returns the session ID associated with this message, if any.
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::UserInput { session_id, .. }
            | Self::AssistantOutput { session_id, .. }
            | Self::ToolUse { session_id, .. }
            | Self::ToolResult { session_id, .. }
            | Self::SessionStart { session_id }
            | Self::SessionEnd { session_id } => Some(session_id),
            Self::Heartbeat | Self::Error { .. } => None,
        }
    }
}

/// The sending half of a message queue.
#[derive(Debug, Clone)]
pub struct MessageQueue {
    tx: mpsc::Sender<BridgeMessage>,
}

/// The receiving half of a message queue.
pub struct MessageQueueReceiver {
    rx: mpsc::Receiver<BridgeMessage>,
}

impl MessageQueue {
    /// Create a new message queue pair with the given buffer capacity.
    pub fn new(capacity: usize) -> (Self, MessageQueueReceiver) {
        let (tx, rx) = mpsc::channel(capacity);
        (Self { tx }, MessageQueueReceiver { rx })
    }

    /// Send a message into the queue.
    pub async fn send(&self, msg: BridgeMessage) -> CcResult<()> {
        self.tx.send(msg).await.map_err(|e| {
            CcError::Internal(format!("Failed to send bridge message: {e}"))
        })
    }

    /// Returns `true` if the receiver has been dropped.
    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}

impl MessageQueueReceiver {
    /// Receive the next message from the queue.
    ///
    /// Returns `None` if all senders have been dropped.
    pub async fn recv(&mut self) -> Option<BridgeMessage> {
        self.rx.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_deserialize_roundtrip() {
        let msg = BridgeMessage::UserInput {
            text: "Hello".into(),
            session_id: "sess-1".into(),
        };
        let json = msg.serialize().unwrap();
        let decoded = BridgeMessage::deserialize(&json).unwrap();
        assert_eq!(msg, decoded);
    }

    #[test]
    fn heartbeat_roundtrip() {
        let msg = BridgeMessage::Heartbeat;
        let json = msg.serialize().unwrap();
        let decoded = BridgeMessage::deserialize(&json).unwrap();
        assert_eq!(decoded, BridgeMessage::Heartbeat);
    }

    #[test]
    fn session_id_extraction() {
        let msg = BridgeMessage::ToolUse {
            tool_name: "bash".into(),
            input: serde_json::json!({}),
            session_id: "s42".into(),
        };
        assert_eq!(msg.session_id(), Some("s42"));
        assert_eq!(BridgeMessage::Heartbeat.session_id(), None);
    }

    #[test]
    fn error_message_has_no_session() {
        let msg = BridgeMessage::Error {
            code: "E001".into(),
            message: "something broke".into(),
        };
        assert_eq!(msg.session_id(), None);
    }

    #[tokio::test]
    async fn message_queue_send_recv() {
        let (queue, mut receiver) = MessageQueue::new(16);
        let msg = BridgeMessage::Heartbeat;
        queue.send(msg.clone()).await.unwrap();
        let received = receiver.recv().await.unwrap();
        assert_eq!(received, BridgeMessage::Heartbeat);
    }

    #[tokio::test]
    async fn message_queue_closed_after_drop() {
        let (queue, receiver) = MessageQueue::new(4);
        drop(receiver);
        assert!(queue.is_closed());
    }
}
