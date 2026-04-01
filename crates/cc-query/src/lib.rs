//! The agentic query loop for Claude Code RS.
//!
//! This is the heart of the application. It sends messages to the Claude
//! API, executes tool calls, and loops until the model signals completion
//! or the turn limit is reached.

pub mod system_prompt;
pub mod tool_prompts;

use cc_api::streaming::{simplify_stream, StreamOutput};
use cc_api::types::{ApiContent, ApiMessage, ContentBlock};
use cc_api::{ApiClient, CreateMessageRequest};
use cc_cost::{CallUsage, CostTracker};
use cc_tools_core::{ToolCall, ToolContext, ToolExecutor};
use tokio_stream::{Stream, StreamExt};

/// Events emitted by the query loop, consumed by the UI layer.
#[derive(Debug, Clone)]
pub enum QueryEvent {
    /// Incremental text from the assistant.
    Text(String),
    /// Incremental thinking/reasoning text.
    Thinking(String),
    /// A tool use has started.
    ToolUseStart { name: String, id: String },
    /// A tool has produced a result.
    ToolResult {
        id: String,
        output: String,
        is_error: bool,
    },
    /// A single API turn completed.
    TurnComplete {
        stop_reason: String,
        input_tokens: u64,
        output_tokens: u64,
    },
    /// A non-fatal error occurred.
    Error(String),
    /// The entire query loop has finished.
    Done {
        total_turns: u32,
        total_cost: String,
    },
}

/// Configuration for creating a [`QueryLoop`].
pub struct QueryLoopConfig {
    /// The API client to use for sending requests.
    pub api_client: ApiClient,
    /// The executor that runs tool calls.
    pub tool_executor: ToolExecutor,
    /// The context (working dir, permissions) for tool execution.
    pub tool_context: ToolContext,
    /// The model identifier string (e.g. "claude-sonnet-4-20250514").
    pub model: String,
    /// Maximum tokens the model may produce per turn.
    pub max_tokens: u32,
    /// Maximum number of agentic turns before stopping.
    pub max_turns: u32,
}

/// The main agentic query loop.
///
/// Holds the conversation state and drives the send-execute-loop cycle.
pub struct QueryLoop {
    api_client: ApiClient,
    tool_executor: ToolExecutor,
    tool_context: ToolContext,
    cost_tracker: CostTracker,
    messages: Vec<ApiMessage>,
    system_prompt: Option<String>,
    model: String,
    max_tokens: u32,
    max_turns: u32,
}

impl QueryLoop {
    /// Create a new query loop from configuration.
    pub fn new(config: QueryLoopConfig) -> Self {
        Self {
            api_client: config.api_client,
            tool_executor: config.tool_executor,
            tool_context: config.tool_context,
            cost_tracker: CostTracker::new(),
            messages: Vec::new(),
            system_prompt: None,
            model: config.model,
            max_tokens: config.max_tokens,
            max_turns: config.max_turns,
        }
    }

    /// Set the system prompt for the conversation.
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = Some(prompt);
    }

    /// Get a reference to the conversation messages.
    pub fn messages(&self) -> &[ApiMessage] {
        &self.messages
    }

    /// Get a reference to the cost tracker.
    pub fn cost_tracker(&self) -> &CostTracker {
        &self.cost_tracker
    }

    /// Run the full agentic loop for a user prompt.
    ///
    /// Returns a stream of [`QueryEvent`]s. The loop:
    /// 1. Adds the user message to the conversation.
    /// 2. Sends the conversation to the API (streaming).
    /// 3. Collects the full assistant response.
    /// 4. If the model requested tool use, executes the tools,
    ///    adds results to the conversation, and loops back to step 2.
    /// 5. If the model signals `end_turn` or the turn limit is reached,
    ///    yields `QueryEvent::Done` and returns.
    pub fn run(&mut self, user_prompt: &str) -> impl Stream<Item = QueryEvent> + '_ {
        let user_prompt = user_prompt.to_string();

        async_stream::stream! {
            // Step 1: Add user message.
            self.messages.push(ApiMessage {
                role: "user".to_string(),
                content: ApiContent::Text(user_prompt),
            });

            let mut turn = 0u32;

            loop {
                turn += 1;
                if turn > self.max_turns {
                    tracing::warn!(max_turns = self.max_turns, "turn limit reached");
                    yield QueryEvent::Error(format!(
                        "Reached maximum turn limit ({})",
                        self.max_turns
                    ));
                    break;
                }

                tracing::info!(turn, "starting API turn");

                // Step 2: Build and send the API request.
                let tool_defs = self.tool_executor.registry().to_api_tools();

                let request = CreateMessageRequest {
                    model: self.model.clone(),
                    messages: self.messages.clone(),
                    max_tokens: self.max_tokens,
                    system: self.system_prompt.clone(),
                    tools: if tool_defs.is_empty() {
                        None
                    } else {
                        Some(tool_defs)
                    },
                    temperature: None,
                    stop_sequences: None,
                };

                let raw_stream = match self.api_client.send_streaming(&request).await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!(error = %e, "API request failed");
                        yield QueryEvent::Error(format!("API error: {}", e));
                        break;
                    }
                };

                // Step 3: Process the stream, collecting the full response.
                let simplified = simplify_stream(raw_stream);
                tokio::pin!(simplified);

                let mut text_parts: Vec<String> = Vec::new();
                let mut tool_uses: Vec<PendingToolUse> = Vec::new();
                let mut stop_reason = String::new();
                let mut input_tokens = 0u64;
                let mut output_tokens = 0u64;

                while let Some(event) = simplified.next().await {
                    match event {
                        StreamOutput::Text(text) => {
                            text_parts.push(text.clone());
                            yield QueryEvent::Text(text);
                        }
                        StreamOutput::Thinking(thinking) => {
                            yield QueryEvent::Thinking(thinking);
                        }
                        StreamOutput::ToolUse { id, name, input } => {
                            yield QueryEvent::ToolUseStart {
                                name: name.clone(),
                                id: id.clone(),
                            };
                            tool_uses.push(PendingToolUse { id, name, input });
                        }
                        StreamOutput::Done {
                            stop_reason: sr,
                            input_tokens: it,
                            output_tokens: ot,
                        } => {
                            stop_reason = sr.unwrap_or_else(|| "end_turn".to_string());
                            input_tokens = it;
                            output_tokens = ot;
                        }
                        StreamOutput::Error(msg) => {
                            yield QueryEvent::Error(msg);
                        }
                    }
                }

                // Record cost.
                self.cost_tracker.record(
                    &self.model,
                    &CallUsage {
                        input_tokens,
                        output_tokens,
                        cache_creation_tokens: 0,
                        cache_read_tokens: 0,
                    },
                );

                yield QueryEvent::TurnComplete {
                    stop_reason: stop_reason.clone(),
                    input_tokens,
                    output_tokens,
                };

                // Step 4: Build the assistant message from collected content.
                let mut assistant_blocks: Vec<ContentBlock> = Vec::new();

                let full_text: String = text_parts.join("");
                if !full_text.is_empty() {
                    assistant_blocks.push(ContentBlock::Text { text: full_text });
                }

                for tu in &tool_uses {
                    assistant_blocks.push(ContentBlock::ToolUse {
                        id: tu.id.clone(),
                        name: tu.name.clone(),
                        input: tu.input.clone(),
                    });
                }

                if !assistant_blocks.is_empty() {
                    self.messages.push(ApiMessage {
                        role: "assistant".to_string(),
                        content: ApiContent::Blocks(assistant_blocks),
                    });
                }

                // Step 5: If tool_use, execute tools and loop.
                if stop_reason == "tool_use" && !tool_uses.is_empty() {
                    let tool_calls: Vec<ToolCall> = tool_uses
                        .iter()
                        .map(|tu| ToolCall {
                            id: tu.id.clone(),
                            name: tu.name.clone(),
                            input: tu.input.clone(),
                        })
                        .collect();

                    let results = self
                        .tool_executor
                        .execute_batch(tool_calls, &self.tool_context)
                        .await;

                    // Build tool result content blocks.
                    let mut result_blocks: Vec<ContentBlock> = Vec::new();
                    for (call_id, result) in results {
                        let (output_text, is_error) = match result {
                            Ok(output) => (output.content.clone(), output.is_error),
                            Err(e) => (format!("Error: {}", e), true),
                        };

                        yield QueryEvent::ToolResult {
                            id: call_id.clone(),
                            output: output_text.clone(),
                            is_error,
                        };

                        result_blocks.push(ContentBlock::ToolResult {
                            tool_use_id: call_id,
                            content: serde_json::Value::String(output_text),
                            is_error,
                        });
                    }

                    // Add tool results as a user message.
                    self.messages.push(ApiMessage {
                        role: "user".to_string(),
                        content: ApiContent::Blocks(result_blocks),
                    });

                    // Loop back to step 2.
                    continue;
                }

                // Step 6: end_turn or max_tokens => we're done.
                break;
            }

            yield QueryEvent::Done {
                total_turns: turn,
                total_cost: self.cost_tracker.format_cost(),
            };
        }
    }
}

/// Internal struct to hold a pending tool use before execution.
struct PendingToolUse {
    id: String,
    name: String,
    input: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_event_debug() {
        // Verify QueryEvent derives Debug correctly.
        let event = QueryEvent::Text("hello".to_string());
        let debug = format!("{:?}", event);
        assert!(debug.contains("hello"));
    }

    #[test]
    fn query_event_done() {
        let event = QueryEvent::Done {
            total_turns: 3,
            total_cost: "$0.0123".to_string(),
        };
        if let QueryEvent::Done {
            total_turns,
            total_cost,
        } = event
        {
            assert_eq!(total_turns, 3);
            assert!(total_cost.contains("0.0123"));
        }
    }

    #[test]
    fn test_query_event_variants() {
        // Verify every QueryEvent variant can be created and debug-printed.
        let events: Vec<QueryEvent> = vec![
            QueryEvent::Text("hello".into()),
            QueryEvent::Thinking("reasoning...".into()),
            QueryEvent::ToolUseStart {
                name: "Bash".into(),
                id: "toolu_123".into(),
            },
            QueryEvent::ToolResult {
                id: "toolu_123".into(),
                output: "file list".into(),
                is_error: false,
            },
            QueryEvent::TurnComplete {
                stop_reason: "end_turn".into(),
                input_tokens: 100,
                output_tokens: 50,
            },
            QueryEvent::Error("something went wrong".into()),
            QueryEvent::Done {
                total_turns: 2,
                total_cost: "$0.01".into(),
            },
        ];

        // All variants should format via Debug without panicking.
        for event in &events {
            let debug = format!("{:?}", event);
            assert!(!debug.is_empty());
        }

        // Verify each variant matches the expected discriminant.
        assert!(matches!(events[0], QueryEvent::Text(_)));
        assert!(matches!(events[1], QueryEvent::Thinking(_)));
        assert!(matches!(events[2], QueryEvent::ToolUseStart { .. }));
        assert!(matches!(events[3], QueryEvent::ToolResult { .. }));
        assert!(matches!(events[4], QueryEvent::TurnComplete { .. }));
        assert!(matches!(events[5], QueryEvent::Error(_)));
        assert!(matches!(events[6], QueryEvent::Done { .. }));
    }

    #[test]
    fn test_query_loop_config() {
        use cc_api::{ApiClient, ApiClientConfig};
        use cc_tools_core::{ToolContext, ToolExecutor, ToolRegistry};
        use cc_permissions::PermissionContext;
        use std::path::PathBuf;
        use std::sync::Arc;

        let api_client = ApiClient::new(ApiClientConfig {
            api_key: "test-key".into(),
            ..Default::default()
        })
        .expect("should build API client");

        let registry = ToolRegistry::new();
        let executor = ToolExecutor::new(Arc::new(registry));

        let tool_context = ToolContext {
            working_directory: PathBuf::from("."),
            permission_context: PermissionContext::new(
                cc_permissions::PermissionMode::Bypass,
                vec![],
            ),
        };

        let config = QueryLoopConfig {
            api_client,
            tool_executor: executor,
            tool_context,
            model: "claude-sonnet-4-20250514".into(),
            max_tokens: 4096,
            max_turns: 10,
        };

        assert_eq!(config.model, "claude-sonnet-4-20250514");
        assert_eq!(config.max_tokens, 4096);
        assert_eq!(config.max_turns, 10);

        // Verify QueryLoop can be created from config.
        let query_loop = QueryLoop::new(config);
        assert!(query_loop.messages().is_empty());
    }
}
