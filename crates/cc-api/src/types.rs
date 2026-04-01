//! API request and response types matching the Anthropic Messages API.

use serde::{Deserialize, Serialize};

/// A message in the API format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMessage {
    pub role: String,
    pub content: ApiContent,
}

/// Content can be a simple string or an array of content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApiContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// A single content block in the API format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: serde_json::Value,
        #[serde(default)]
        is_error: bool,
    },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
    },
}

/// Non-streaming API response.
#[derive(Debug, Clone, Deserialize)]
pub struct MessageResponse {
    pub id: String,
    pub role: String,
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: Usage,
}

/// Token usage in API response.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u64>,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u64>,
}

/// API error response body.
#[derive(Debug, Deserialize)]
pub struct ApiErrorBody {
    pub error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct ApiErrorDetail {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

/// Server-Sent Event types from the streaming API.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// message_start: initial message metadata
    MessageStart {
        id: String,
        model: String,
        usage: Usage,
    },
    /// content_block_start: a new content block begins
    ContentBlockStart {
        index: usize,
        content_block: ContentBlock,
    },
    /// content_block_delta: incremental text or tool input
    ContentBlockDelta {
        index: usize,
        delta: Delta,
    },
    /// content_block_stop: a content block finished
    ContentBlockStop {
        index: usize,
    },
    /// message_delta: final message metadata (stop_reason, usage)
    MessageDelta {
        stop_reason: Option<String>,
        usage: Usage,
    },
    /// message_stop: stream complete
    MessageStop,
    /// ping: keepalive
    Ping,
    /// error: server error during streaming
    Error {
        error_type: String,
        message: String,
    },
}

/// Delta content within a content_block_delta event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Delta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { thinking: String },
}
