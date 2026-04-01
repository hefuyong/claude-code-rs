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
}
