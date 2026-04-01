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
fn parse_sse_message(message: &str) -> Option<CcResult<StreamEvent>> {
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
