//! High-level streaming helpers that convert raw SSE events
//! into a user-friendly stream of text chunks and metadata.

use crate::types::{Delta, StreamEvent};
use cc_error::CcResult;
use tokio_stream::{Stream, StreamExt};

/// A simplified output event from the streaming API.
#[derive(Debug, Clone)]
pub enum StreamOutput {
    /// Incremental text from the assistant.
    Text(String),
    /// Incremental thinking text (extended thinking).
    Thinking(String),
    /// A tool use was requested.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// The message is complete.
    Done {
        stop_reason: Option<String>,
        input_tokens: u64,
        output_tokens: u64,
    },
    /// A streaming error occurred.
    Error(String),
}

/// Convert a raw SSE stream into a stream of simplified `StreamOutput` events.
pub fn simplify_stream(
    raw: impl Stream<Item = CcResult<StreamEvent>>,
) -> impl Stream<Item = StreamOutput> {
    async_stream::stream! {
        tokio::pin!(raw);

        let mut current_tool_id = String::new();
        let mut current_tool_name = String::new();
        let mut current_tool_json = String::new();
        let mut total_input_tokens = 0u64;
        let mut total_output_tokens = 0u64;

        while let Some(event) = raw.next().await {
            match event {
                Err(e) => {
                    yield StreamOutput::Error(e.to_string());
                    break;
                }
                Ok(StreamEvent::MessageStart { usage, .. }) => {
                    total_input_tokens += usage.input_tokens;
                    total_output_tokens += usage.output_tokens;
                }
                Ok(StreamEvent::ContentBlockStart {
                    content_block: crate::types::ContentBlock::ToolUse { id, name, .. },
                    ..
                }) => {
                    current_tool_id = id;
                    current_tool_name = name;
                    current_tool_json.clear();
                }
                Ok(StreamEvent::ContentBlockDelta { delta, .. }) => match delta {
                    Delta::TextDelta { text } => {
                        yield StreamOutput::Text(text);
                    }
                    Delta::ThinkingDelta { thinking } => {
                        yield StreamOutput::Thinking(thinking);
                    }
                    Delta::InputJsonDelta { partial_json } => {
                        current_tool_json.push_str(&partial_json);
                    }
                },
                Ok(StreamEvent::ContentBlockStop { .. }) => {
                    if !current_tool_name.is_empty() {
                        let input = serde_json::from_str(&current_tool_json)
                            .unwrap_or(serde_json::Value::Object(Default::default()));
                        yield StreamOutput::ToolUse {
                            id: std::mem::take(&mut current_tool_id),
                            name: std::mem::take(&mut current_tool_name),
                            input,
                        };
                        current_tool_json.clear();
                    }
                }
                Ok(StreamEvent::MessageDelta {
                    stop_reason,
                    usage,
                }) => {
                    total_output_tokens += usage.output_tokens;
                    yield StreamOutput::Done {
                        stop_reason,
                        input_tokens: total_input_tokens,
                        output_tokens: total_output_tokens,
                    };
                }
                Ok(StreamEvent::Error {
                    message, ..
                }) => {
                    yield StreamOutput::Error(message);
                }
                Ok(StreamEvent::MessageStop | StreamEvent::Ping) => {}
                Ok(StreamEvent::ContentBlockStart { .. }) => {}
            }
        }
    }
}
