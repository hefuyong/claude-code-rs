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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ContentBlock, Usage};
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_simplify_text_output() {
        let events: Vec<CcResult<StreamEvent>> = vec![
            Ok(StreamEvent::MessageStart {
                id: "msg_1".into(),
                model: "claude-sonnet-4-20250514".into(),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 0,
                    ..Default::default()
                },
            }),
            Ok(StreamEvent::ContentBlockStart {
                index: 0,
                content_block: ContentBlock::Text {
                    text: String::new(),
                },
            }),
            Ok(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: Delta::TextDelta {
                    text: "Hello ".into(),
                },
            }),
            Ok(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: Delta::TextDelta {
                    text: "world".into(),
                },
            }),
            Ok(StreamEvent::ContentBlockStop { index: 0 }),
            Ok(StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".into()),
                usage: Usage {
                    input_tokens: 0,
                    output_tokens: 5,
                    ..Default::default()
                },
            }),
            Ok(StreamEvent::MessageStop),
        ];

        let raw = tokio_stream::iter(events);
        let simplified = simplify_stream(raw);
        tokio::pin!(simplified);

        let mut texts = Vec::new();
        let mut done_seen = false;

        while let Some(event) = simplified.next().await {
            match event {
                StreamOutput::Text(t) => texts.push(t),
                StreamOutput::Done { stop_reason, input_tokens, output_tokens } => {
                    assert_eq!(stop_reason.as_deref(), Some("end_turn"));
                    assert_eq!(input_tokens, 10);
                    assert_eq!(output_tokens, 5);
                    done_seen = true;
                }
                _ => {}
            }
        }

        assert_eq!(texts, vec!["Hello ", "world"]);
        assert!(done_seen, "should have received Done event");
    }

    #[tokio::test]
    async fn test_simplify_tool_use() {
        let events: Vec<CcResult<StreamEvent>> = vec![
            Ok(StreamEvent::MessageStart {
                id: "msg_2".into(),
                model: "claude-sonnet-4-20250514".into(),
                usage: Usage::default(),
            }),
            Ok(StreamEvent::ContentBlockStart {
                index: 0,
                content_block: ContentBlock::ToolUse {
                    id: "toolu_123".into(),
                    name: "Bash".into(),
                    input: serde_json::json!({}),
                },
            }),
            Ok(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: Delta::InputJsonDelta {
                    partial_json: "{\"command\":".into(),
                },
            }),
            Ok(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: Delta::InputJsonDelta {
                    partial_json: "\"ls -la\"}".into(),
                },
            }),
            Ok(StreamEvent::ContentBlockStop { index: 0 }),
            Ok(StreamEvent::MessageDelta {
                stop_reason: Some("tool_use".into()),
                usage: Usage {
                    input_tokens: 0,
                    output_tokens: 20,
                    ..Default::default()
                },
            }),
            Ok(StreamEvent::MessageStop),
        ];

        let raw = tokio_stream::iter(events);
        let simplified = simplify_stream(raw);
        tokio::pin!(simplified);

        let mut tool_use_seen = false;

        while let Some(event) = simplified.next().await {
            if let StreamOutput::ToolUse { id, name, input } = event {
                assert_eq!(id, "toolu_123");
                assert_eq!(name, "Bash");
                assert_eq!(input["command"], "ls -la");
                tool_use_seen = true;
            }
        }

        assert!(tool_use_seen, "should have received ToolUse event");
    }

    #[tokio::test]
    async fn test_simplify_done() {
        let events: Vec<CcResult<StreamEvent>> = vec![
            Ok(StreamEvent::MessageStart {
                id: "msg_3".into(),
                model: "claude-sonnet-4-20250514".into(),
                usage: Usage {
                    input_tokens: 50,
                    output_tokens: 0,
                    ..Default::default()
                },
            }),
            Ok(StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".into()),
                usage: Usage {
                    input_tokens: 0,
                    output_tokens: 30,
                    ..Default::default()
                },
            }),
            Ok(StreamEvent::MessageStop),
        ];

        let raw = tokio_stream::iter(events);
        let simplified = simplify_stream(raw);
        tokio::pin!(simplified);

        let mut done_event = None;

        while let Some(event) = simplified.next().await {
            if let StreamOutput::Done { stop_reason, input_tokens, output_tokens } = event {
                done_event = Some((stop_reason, input_tokens, output_tokens));
            }
        }

        let (stop_reason, input_tokens, output_tokens) = done_event.expect("should have Done");
        assert_eq!(stop_reason.as_deref(), Some("end_turn"));
        assert_eq!(input_tokens, 50);
        assert_eq!(output_tokens, 30);
    }
}
