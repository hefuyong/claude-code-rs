//! Server-Sent Events (SSE) parser for Anthropic streaming API.

use crate::types::{ContentBlock, Delta, StreamEvent, Usage};
use cc_error::{CcError, CcResult};
use futures::StreamExt;
use tokio_stream::Stream;

/// Parse an HTTP response body as an SSE stream of `StreamEvent`s.
pub fn parse_sse_stream(
    response: reqwest::Response,
) -> impl Stream<Item = CcResult<StreamEvent>> {
    async_stream::stream! {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    yield Err(CcError::Api {
                        message: format!("stream read error: {e}"),
                        status_code: None,
                    });
                    break;
                }
            };

            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete SSE messages (double newline separated)
            while let Some(pos) = buffer.find("\n\n") {
                let message = buffer[..pos].to_string();
                buffer = buffer[pos + 2..].to_string();

                if let Some(event) = parse_sse_message(&message) {
                    yield event;
                }
            }
        }

        // Process any remaining data
        if !buffer.trim().is_empty() {
            if let Some(event) = parse_sse_message(&buffer) {
                yield event;
            }
        }
    }
}

/// Parse a single SSE message block into a StreamEvent.
pub(crate) fn parse_sse_message(message: &str) -> Option<CcResult<StreamEvent>> {
    let mut event_type = String::new();
    let mut data = String::new();

    for line in message.lines() {
        if let Some(rest) = line.strip_prefix("event: ") {
            event_type = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data: ") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest);
        }
    }

    if event_type.is_empty() && data.is_empty() {
        return None;
    }

    Some(parse_event(&event_type, &data))
}

fn parse_event(event_type: &str, data: &str) -> CcResult<StreamEvent> {
    match event_type {
        "message_start" => {
            let v: serde_json::Value =
                serde_json::from_str(data).map_err(|e| sse_parse_err(e, data))?;
            let msg = &v["message"];
            Ok(StreamEvent::MessageStart {
                id: msg["id"].as_str().unwrap_or("").to_string(),
                model: msg["model"].as_str().unwrap_or("").to_string(),
                usage: serde_json::from_value(msg["usage"].clone()).unwrap_or_default(),
            })
        }
        "content_block_start" => {
            let v: serde_json::Value =
                serde_json::from_str(data).map_err(|e| sse_parse_err(e, data))?;
            let index = v["index"].as_u64().unwrap_or(0) as usize;
            let block: ContentBlock =
                serde_json::from_value(v["content_block"].clone())
                    .map_err(|e| sse_parse_err(e, data))?;
            Ok(StreamEvent::ContentBlockStart {
                index,
                content_block: block,
            })
        }
        "content_block_delta" => {
            let v: serde_json::Value =
                serde_json::from_str(data).map_err(|e| sse_parse_err(e, data))?;
            let index = v["index"].as_u64().unwrap_or(0) as usize;
            let delta: Delta =
                serde_json::from_value(v["delta"].clone())
                    .map_err(|e| sse_parse_err(e, data))?;
            Ok(StreamEvent::ContentBlockDelta { index, delta })
        }
        "content_block_stop" => {
            let v: serde_json::Value =
                serde_json::from_str(data).map_err(|e| sse_parse_err(e, data))?;
            let index = v["index"].as_u64().unwrap_or(0) as usize;
            Ok(StreamEvent::ContentBlockStop { index })
        }
        "message_delta" => {
            let v: serde_json::Value =
                serde_json::from_str(data).map_err(|e| sse_parse_err(e, data))?;
            let stop_reason = v["delta"]["stop_reason"].as_str().map(String::from);
            let usage: Usage =
                serde_json::from_value(v["usage"].clone()).unwrap_or_default();
            Ok(StreamEvent::MessageDelta { stop_reason, usage })
        }
        "message_stop" => Ok(StreamEvent::MessageStop),
        "ping" => Ok(StreamEvent::Ping),
        "error" => {
            let v: serde_json::Value =
                serde_json::from_str(data).map_err(|e| sse_parse_err(e, data))?;
            Ok(StreamEvent::Error {
                error_type: v["error"]["type"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string(),
                message: v["error"]["message"]
                    .as_str()
                    .unwrap_or(data)
                    .to_string(),
            })
        }
        _ => {
            tracing::debug!(event_type, "unknown SSE event type, ignoring");
            Ok(StreamEvent::Ping) // treat unknown as no-op
        }
    }
}

fn sse_parse_err(e: impl std::fmt::Display, data: &str) -> CcError {
    CcError::Serialization(format!("SSE parse error: {e}, data: {}", &data[..data.len().min(200)]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_text_delta() {
        let msg = "event: content_block_delta\n\
                   data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello world\"}}";
        let result = parse_sse_message(msg);
        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                match delta {
                    Delta::TextDelta { text } => assert_eq!(text, "Hello world"),
                    other => panic!("expected TextDelta, got {:?}", other),
                }
            }
            other => panic!("expected ContentBlockDelta, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_message_start() {
        let msg = "event: message_start\n\
                   data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-sonnet-4-20250514\",\"usage\":{\"input_tokens\":100,\"output_tokens\":0}}}";
        let result = parse_sse_message(msg);
        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            StreamEvent::MessageStart { id, model, usage } => {
                assert_eq!(id, "msg_123");
                assert_eq!(model, "claude-sonnet-4-20250514");
                assert_eq!(usage.input_tokens, 100);
                assert_eq!(usage.output_tokens, 0);
            }
            other => panic!("expected MessageStart, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_message_stop() {
        let msg = "event: message_stop\ndata: {}";
        let result = parse_sse_message(msg);
        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        assert!(matches!(event, StreamEvent::MessageStop));
    }

    #[test]
    fn test_parse_tool_use_block() {
        let msg = "event: content_block_start\n\
                   data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_abc\",\"name\":\"Bash\",\"input\":{}}}";
        let result = parse_sse_message(msg);
        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            StreamEvent::ContentBlockStart { index, content_block } => {
                assert_eq!(index, 1);
                match content_block {
                    ContentBlock::ToolUse { id, name, .. } => {
                        assert_eq!(id, "toolu_abc");
                        assert_eq!(name, "Bash");
                    }
                    other => panic!("expected ToolUse block, got {:?}", other),
                }
            }
            other => panic!("expected ContentBlockStart, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_error_event() {
        let msg = "event: error\n\
                   data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Server is overloaded\"}}";
        let result = parse_sse_message(msg);
        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            StreamEvent::Error { error_type, message } => {
                assert_eq!(error_type, "overloaded_error");
                assert_eq!(message, "Server is overloaded");
            }
            other => panic!("expected Error event, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_unknown_event() {
        let msg = "event: some_future_event\ndata: {\"foo\":\"bar\"}";
        let result = parse_sse_message(msg);
        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        assert!(matches!(event, StreamEvent::Ping));
    }

    #[test]
    fn test_empty_message_ignored() {
        let result = parse_sse_message("");
        assert!(result.is_none());
    }

    #[test]
    fn test_multiline_data() {
        // Simulate data split across multiple data: lines
        let msg = "event: content_block_delta\n\
                   data: {\"type\":\"content_block_delta\",\"index\":0,\n\
                   data: \"delta\":{\"type\":\"text_delta\",\"text\":\"multi\"}}";
        let result = parse_sse_message(msg);
        assert!(result.is_some());
        let event = result.unwrap().unwrap();
        match event {
            StreamEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(index, 0);
                match delta {
                    Delta::TextDelta { text } => assert_eq!(text, "multi"),
                    other => panic!("expected TextDelta, got {:?}", other),
                }
            }
            other => panic!("expected ContentBlockDelta, got {:?}", other),
        }
    }
}
