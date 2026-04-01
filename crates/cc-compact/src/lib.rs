//! Conversation compaction for Claude Code RS.
//!
//! When the conversation history grows too long, this module provides
//! heuristics to decide when to compact and a function that trims
//! the message list while preserving the essential context.

use cc_api::types::{ApiContent, ApiMessage, ContentBlock};

/// Rough estimate of the number of tokens in a string.
/// Uses the common heuristic of ~4 characters per token.
fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

/// Estimate the total token count across all messages.
fn estimate_message_tokens(messages: &[ApiMessage]) -> usize {
    messages
        .iter()
        .map(|msg| match &msg.content {
            ApiContent::Text(t) => estimate_tokens(t),
            ApiContent::Blocks(blocks) => blocks
                .iter()
                .map(|b| match b {
                    ContentBlock::Text { text } => estimate_tokens(text),
                    ContentBlock::ToolUse { input, .. } => {
                        estimate_tokens(&input.to_string())
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        estimate_tokens(&content.to_string())
                    }
                    ContentBlock::Thinking { thinking } => estimate_tokens(thinking),
                })
                .sum(),
        })
        .sum()
}

/// Determine whether the conversation should be compacted.
///
/// Returns `true` when the estimated token count exceeds
/// `max_tokens_estimate` or the message count exceeds 100.
pub fn should_compact(messages: &[ApiMessage], max_tokens_estimate: usize) -> bool {
    if messages.len() > 100 {
        tracing::debug!(
            message_count = messages.len(),
            "compaction triggered by message count"
        );
        return true;
    }
    let tokens = estimate_message_tokens(messages);
    if tokens > max_tokens_estimate {
        tracing::debug!(
            estimated_tokens = tokens,
            threshold = max_tokens_estimate,
            "compaction triggered by token estimate"
        );
        return true;
    }
    false
}

/// Compact a conversation by keeping the first few and last few messages
/// and inserting a summary marker in between.
///
/// Strategy:
/// - Keep the first `keep_start` messages (preserves the initial system
///   context and the user's opening prompt).
/// - Keep the last `keep_end` messages (preserves the most recent context).
/// - Insert a `[conversation compacted]` marker message containing
///   `summary_prompt` between the two retained segments.
///
/// If the conversation is already short enough (fewer than
/// `keep_start + keep_end + 2` messages) it is returned unchanged.
pub fn compact(messages: &[ApiMessage], summary_prompt: &str) -> Vec<ApiMessage> {
    let keep_start: usize = 2;
    let keep_end: usize = 10;

    // Nothing to compact if the conversation is short.
    if messages.len() <= keep_start + keep_end + 2 {
        tracing::debug!(
            message_count = messages.len(),
            "conversation too short to compact, returning as-is"
        );
        return messages.to_vec();
    }

    let mut result = Vec::with_capacity(keep_start + 1 + keep_end);

    // First N messages.
    result.extend_from_slice(&messages[..keep_start]);

    // Compaction marker as a user message so the assistant knows
    // context was truncated.
    let marker_text = if summary_prompt.is_empty() {
        "[conversation compacted — earlier messages were removed to fit context limits]".to_string()
    } else {
        format!(
            "[conversation compacted]\n\nSummary of removed messages:\n{}",
            summary_prompt
        )
    };

    result.push(ApiMessage {
        role: "user".to_string(),
        content: ApiContent::Text(marker_text),
    });

    // Last N messages.
    let tail_start = messages.len() - keep_end;
    result.extend_from_slice(&messages[tail_start..]);

    tracing::info!(
        original = messages.len(),
        compacted = result.len(),
        removed = messages.len() - (keep_start + keep_end),
        "conversation compacted"
    );

    result
}

// ---------------------------------------------------------------------------
// Auto-compact configuration and micro-compact
// ---------------------------------------------------------------------------

use serde::{Deserialize, Serialize};

/// Configuration for automatic compaction triggers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoCompactConfig {
    /// Whether auto-compact is enabled.
    pub enabled: bool,
    /// Trigger a full compact when estimated token count exceeds this value.
    pub token_threshold: usize,
    /// Trigger a full compact when message count exceeds this value.
    pub message_threshold: usize,
    /// Always keep the last N messages unmodified.
    pub keep_recent: usize,
    /// Run micro-compact every N new messages.
    pub microcompact_interval: usize,
}

impl Default for AutoCompactConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            token_threshold: 80_000,
            message_threshold: 100,
            keep_recent: 10,
            microcompact_interval: 20,
        }
    }
}

/// Urgency level for a compaction warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompactUrgency {
    /// Informational — approaching the threshold.
    Info,
    /// Warning — close to the threshold.
    Warning,
    /// Critical — at or above the threshold.
    Critical,
}

/// A warning that compaction should be considered.
#[derive(Debug, Clone)]
pub struct CompactWarning {
    /// Human-readable description.
    pub message: String,
    /// How urgent the warning is.
    pub urgency: CompactUrgency,
    /// Current estimated token count.
    pub estimated_tokens: usize,
    /// The threshold that triggered the warning.
    pub threshold: usize,
}

/// Check whether the conversation is approaching or exceeding compact thresholds.
///
/// Returns `None` if no warning is warranted.
pub fn check_compact_warning(
    messages: &[ApiMessage],
    config: &AutoCompactConfig,
) -> Option<CompactWarning> {
    if !config.enabled {
        return None;
    }

    let tokens = estimate_message_tokens(messages);
    let msg_count = messages.len();

    // Critical: at or above threshold.
    if tokens >= config.token_threshold || msg_count >= config.message_threshold {
        return Some(CompactWarning {
            message: format!(
                "Conversation is very large ({} tokens, {} messages). \
                 Compaction is strongly recommended.",
                tokens, msg_count,
            ),
            urgency: CompactUrgency::Critical,
            estimated_tokens: tokens,
            threshold: config.token_threshold,
        });
    }

    // Warning: above 80% of threshold.
    let token_pct = (tokens as f64) / (config.token_threshold as f64);
    let msg_pct = (msg_count as f64) / (config.message_threshold as f64);
    if token_pct > 0.8 || msg_pct > 0.8 {
        return Some(CompactWarning {
            message: format!(
                "Conversation is getting large ({} tokens, {} messages). \
                 Consider compacting soon.",
                tokens, msg_count,
            ),
            urgency: CompactUrgency::Warning,
            estimated_tokens: tokens,
            threshold: config.token_threshold,
        });
    }

    // Info: above 60%.
    if token_pct > 0.6 || msg_pct > 0.6 {
        return Some(CompactWarning {
            message: format!(
                "Conversation size: {} tokens, {} messages.",
                tokens, msg_count,
            ),
            urgency: CompactUrgency::Info,
            estimated_tokens: tokens,
            threshold: config.token_threshold,
        });
    }

    None
}

/// Incremental micro-compactor.
///
/// Tracks message count and periodically compresses older messages by
/// trimming detail while preserving structure.
pub struct MicroCompact {
    config: AutoCompactConfig,
    message_count_since_last: usize,
}

impl MicroCompact {
    /// Create a new micro-compactor.
    pub fn new(config: AutoCompactConfig) -> Self {
        Self {
            config,
            message_count_since_last: 0,
        }
    }

    /// Whether a micro-compact pass should run now.
    pub fn should_run(&self, total_messages: usize, estimated_tokens: usize) -> bool {
        if !self.config.enabled {
            return false;
        }
        if self.message_count_since_last >= self.config.microcompact_interval {
            return true;
        }
        // Also trigger if we're above 70% of the token threshold.
        if estimated_tokens > (self.config.token_threshold * 7) / 10 {
            return total_messages > self.config.keep_recent;
        }
        false
    }

    /// Record that a new message was added to the conversation.
    pub fn record_message(&mut self) {
        self.message_count_since_last += 1;
    }

    /// Perform a micro-compact pass on the given messages.
    ///
    /// Strategy: keep the first 2 messages and the last `keep_recent` messages
    /// intact. For messages in between, collapse consecutive tool-use /
    /// tool-result pairs into a summary and shorten long text blocks.
    pub fn micro_compact(&mut self, messages: &[ApiMessage]) -> Vec<ApiMessage> {
        self.message_count_since_last = 0;

        let keep_recent = self.config.keep_recent;
        if messages.len() <= keep_recent + 2 {
            return messages.to_vec();
        }

        let mut result = Vec::with_capacity(messages.len());

        // Keep first 2 messages as-is (system context + opening).
        let head_count = 2.min(messages.len());
        result.extend_from_slice(&messages[..head_count]);

        let tail_start = messages.len().saturating_sub(keep_recent);

        // Middle messages: compress.
        for msg in &messages[head_count..tail_start] {
            result.push(Self::compress_message(msg));
        }

        // Keep recent messages intact.
        result.extend_from_slice(&messages[tail_start..]);

        tracing::debug!(
            original = messages.len(),
            result = result.len(),
            "micro-compact complete"
        );

        result
    }

    /// Compress a single message by truncating long text blocks and
    /// summarising tool results.
    fn compress_message(msg: &ApiMessage) -> ApiMessage {
        const MAX_TEXT_LEN: usize = 500;

        let content = match &msg.content {
            ApiContent::Text(t) => {
                if t.len() > MAX_TEXT_LEN {
                    ApiContent::Text(format!(
                        "{}... [truncated, {} chars total]",
                        &t[..MAX_TEXT_LEN],
                        t.len()
                    ))
                } else {
                    ApiContent::Text(t.clone())
                }
            }
            ApiContent::Blocks(blocks) => {
                let compressed: Vec<ContentBlock> = blocks
                    .iter()
                    .map(|b| match b {
                        ContentBlock::Text { text } => {
                            if text.len() > MAX_TEXT_LEN {
                                ContentBlock::Text {
                                    text: format!(
                                        "{}... [truncated]",
                                        &text[..MAX_TEXT_LEN]
                                    ),
                                }
                            } else {
                                b.clone()
                            }
                        }
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => {
                            let summary = content.to_string();
                            if summary.len() > MAX_TEXT_LEN {
                                ContentBlock::ToolResult {
                                    tool_use_id: tool_use_id.clone(),
                                    content: serde_json::json!(format!(
                                        "{}... [truncated]",
                                        &summary[..MAX_TEXT_LEN]
                                    )),
                                    is_error: *is_error,
                                }
                            } else {
                                b.clone()
                            }
                        }
                        _ => b.clone(),
                    })
                    .collect();
                ApiContent::Blocks(compressed)
            }
        };

        ApiMessage {
            role: msg.role.clone(),
            content,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_api::types::{ApiContent, ApiMessage};

    fn make_messages(n: usize) -> Vec<ApiMessage> {
        (0..n)
            .map(|i| ApiMessage {
                role: if i % 2 == 0 {
                    "user".to_string()
                } else {
                    "assistant".to_string()
                },
                content: ApiContent::Text(format!("message {}", i)),
            })
            .collect()
    }

    #[test]
    fn short_conversation_not_compacted() {
        let msgs = make_messages(5);
        let result = compact(&msgs, "");
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn long_conversation_compacted() {
        let msgs = make_messages(50);
        let result = compact(&msgs, "summary here");
        // 2 start + 1 marker + 10 end = 13
        assert_eq!(result.len(), 13);

        // Check marker is in the right position
        if let ApiContent::Text(ref t) = result[2].content {
            assert!(t.contains("[conversation compacted]"));
            assert!(t.contains("summary here"));
        } else {
            panic!("expected text marker");
        }

        // Check first two are preserved
        if let ApiContent::Text(ref t) = result[0].content {
            assert_eq!(t, "message 0");
        }
        if let ApiContent::Text(ref t) = result[1].content {
            assert_eq!(t, "message 1");
        }

        // Check last message is the original last
        if let ApiContent::Text(ref t) = result[12].content {
            assert_eq!(t, "message 49");
        }
    }

    #[test]
    fn should_compact_by_count() {
        let msgs = make_messages(101);
        assert!(should_compact(&msgs, 1_000_000));
    }

    #[test]
    fn should_compact_by_tokens() {
        // Each message is ~10 chars => ~2-3 tokens, 10 messages => ~25 tokens
        let msgs = make_messages(10);
        assert!(should_compact(&msgs, 1)); // threshold of 1 token
        assert!(!should_compact(&msgs, 1_000_000));
    }

    // --- Auto-compact tests ---

    #[test]
    fn auto_compact_default_config() {
        let cfg = AutoCompactConfig::default();
        assert!(cfg.enabled);
        assert_eq!(cfg.token_threshold, 80_000);
        assert_eq!(cfg.message_threshold, 100);
        assert_eq!(cfg.keep_recent, 10);
        assert_eq!(cfg.microcompact_interval, 20);
    }

    #[test]
    fn check_warning_critical() {
        let cfg = AutoCompactConfig {
            enabled: true,
            token_threshold: 10,
            message_threshold: 5,
            keep_recent: 2,
            microcompact_interval: 5,
        };
        let msgs = make_messages(10);
        let warning = check_compact_warning(&msgs, &cfg);
        assert!(warning.is_some());
        assert_eq!(warning.unwrap().urgency, CompactUrgency::Critical);
    }

    #[test]
    fn check_warning_none_when_small() {
        let cfg = AutoCompactConfig::default();
        let msgs = make_messages(3);
        assert!(check_compact_warning(&msgs, &cfg).is_none());
    }

    #[test]
    fn check_warning_disabled() {
        let cfg = AutoCompactConfig {
            enabled: false,
            ..AutoCompactConfig::default()
        };
        let msgs = make_messages(200);
        assert!(check_compact_warning(&msgs, &cfg).is_none());
    }

    // --- MicroCompact tests ---

    #[test]
    fn micro_compact_short_conversation_unchanged() {
        let cfg = AutoCompactConfig::default();
        let mut mc = MicroCompact::new(cfg);
        let msgs = make_messages(5);
        let result = mc.micro_compact(&msgs);
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn micro_compact_truncates_long_text() {
        let cfg = AutoCompactConfig {
            keep_recent: 2,
            ..AutoCompactConfig::default()
        };
        let mut mc = MicroCompact::new(cfg);

        let long_text = "x".repeat(1000);
        let msgs = vec![
            ApiMessage {
                role: "user".to_string(),
                content: ApiContent::Text("first".into()),
            },
            ApiMessage {
                role: "assistant".to_string(),
                content: ApiContent::Text("second".into()),
            },
            ApiMessage {
                role: "user".to_string(),
                content: ApiContent::Text(long_text),
            },
            ApiMessage {
                role: "assistant".to_string(),
                content: ApiContent::Text("fourth".into()),
            },
            ApiMessage {
                role: "user".to_string(),
                content: ApiContent::Text("fifth".into()),
            },
        ];

        let result = mc.micro_compact(&msgs);
        // The 3rd message (index 2) is in the middle zone and should be truncated.
        if let ApiContent::Text(ref t) = result[2].content {
            assert!(t.contains("[truncated"));
            assert!(t.len() < 600);
        } else {
            panic!("expected text");
        }
    }

    #[test]
    fn micro_compact_should_run_after_interval() {
        let cfg = AutoCompactConfig {
            microcompact_interval: 3,
            ..AutoCompactConfig::default()
        };
        let mut mc = MicroCompact::new(cfg);
        assert!(!mc.should_run(10, 0));
        mc.record_message();
        mc.record_message();
        mc.record_message();
        assert!(mc.should_run(10, 0));
    }

    #[test]
    fn micro_compact_resets_counter() {
        let cfg = AutoCompactConfig {
            microcompact_interval: 2,
            ..AutoCompactConfig::default()
        };
        let mut mc = MicroCompact::new(cfg);
        mc.record_message();
        mc.record_message();
        assert!(mc.should_run(10, 0));

        let msgs = make_messages(20);
        let _ = mc.micro_compact(&msgs);
        // Counter should be reset.
        assert!(!mc.should_run(10, 0));
    }
}
