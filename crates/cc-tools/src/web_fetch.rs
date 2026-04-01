//! WebFetchTool -- fetch URL content via reqwest.

use async_trait::async_trait;
use cc_error::CcError;
use cc_tools_core::{Tool, ToolContext, ToolOutput};
use std::time::Duration;

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "Fetch content from a URL and return it as text"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch"
                },
                "prompt": {
                    "type": "string",
                    "description": "Optional prompt describing what to extract"
                }
            },
            "required": ["url"]
        })
    }

    async fn call(
        &self,
        input: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolOutput, CcError> {
        let url = input
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CcError::tool("web_fetch", "Missing required field: url"))?;

        let _prompt = input.get("prompt").and_then(|v| v.as_str());

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .map_err(|e| CcError::tool("web_fetch", format!("HTTP client error: {}", e)))?;

        let response = client
            .get(url)
            .header("User-Agent", "ClaudeCode/0.1")
            .send()
            .await
            .map_err(|e| CcError::tool("web_fetch", format!("Fetch failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            return Ok(ToolOutput::error(format!(
                "HTTP {} for {}",
                status.as_u16(),
                url
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| CcError::tool("web_fetch", format!("Failed to read body: {}", e)))?;

        // Simple HTML tag stripping.
        let text = strip_html_tags(&body);

        // Truncate very long responses.
        let max_chars = 100_000;
        let output = if text.len() > max_chars {
            format!(
                "{}\n\n... (truncated, {} total characters)",
                &text[..max_chars],
                text.len()
            )
        } else {
            text
        };

        Ok(ToolOutput::success(output))
    }

    fn is_read_only(&self) -> bool {
        true
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
}

/// Very simple HTML tag stripper. Removes anything between < and >,
/// collapses runs of whitespace, and decodes basic HTML entities.
fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut last_was_space = false;

    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            // Emit a space after closing a tag to avoid words merging.
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
            continue;
        }
        if in_tag {
            continue;
        }
        if ch.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }

    // Decode basic entities.
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
