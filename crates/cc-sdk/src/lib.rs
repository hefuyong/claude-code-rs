//! SDK client for programmatic use of Claude Code RS.
//!
//! Wraps the query loop, API client, and tool executor into a
//! simple interface for embedding Claude Code capabilities in
//! other Rust applications.

use cc_api::{ApiClient, ApiClientConfig};
use cc_config::AppConfig;
use cc_error::{CcError, CcResult};
use cc_permissions::{PermissionContext, PermissionMode};
use cc_query::{QueryEvent, QueryLoop, QueryLoopConfig};
use cc_tools_core::{ToolContext, ToolExecutor, ToolRegistry};
use std::path::PathBuf;
use std::sync::Arc;
use tokio_stream::StreamExt as _;

/// A high-level client for programmatic interaction with Claude.
pub struct SdkClient {
    query_loop: QueryLoop,
}

impl SdkClient {
    /// Create a new SDK client from application configuration.
    pub async fn new(config: AppConfig) -> CcResult<Self> {
        let api_key = config
            .api_key
            .clone()
            .ok_or_else(|| CcError::Auth("API key is required".into()))?;

        let api_config = ApiClientConfig {
            api_key,
            base_url: config.api_base_url.clone(),
            model: config.model.clone(),
            max_retries: config.max_retries,
            request_timeout: std::time::Duration::from_secs(config.request_timeout_secs),
            max_tokens: 16384,
        };

        let api_client = ApiClient::new(api_config)?;

        // Create a minimal tool registry (no tools by default in SDK mode).
        let registry = ToolRegistry::new();
        let executor = ToolExecutor::new(Arc::new(registry));

        let tool_context = ToolContext {
            working_directory: PathBuf::from("."),
            permission_context: PermissionContext::new(PermissionMode::Bypass, vec![]),
        };

        let query_loop = QueryLoop::new(QueryLoopConfig {
            api_client,
            tool_executor: executor,
            tool_context,
            model: config.model.0.clone(),
            max_tokens: 16384,
            max_turns: 10,
        });

        Ok(Self { query_loop })
    }

    /// Send a prompt and collect the full text response.
    pub async fn send(&mut self, prompt: &str) -> CcResult<String> {
        let stream = self.query_loop.run(prompt);
        tokio::pin!(stream);

        let mut text_parts = Vec::new();

        while let Some(event) = stream.next().await {
            match event {
                QueryEvent::Text(t) => text_parts.push(t),
                QueryEvent::Error(e) => {
                    return Err(CcError::Internal(e));
                }
                QueryEvent::Done { .. } => break,
                _ => {} // Ignore other events in SDK mode.
            }
        }

        Ok(text_parts.join(""))
    }

    /// Get a reference to the underlying query loop's cost tracker.
    pub fn cost_tracker(&self) -> &cc_cost::CostTracker {
        self.query_loop.cost_tracker()
    }

    /// Set the system prompt for subsequent queries.
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.query_loop.set_system_prompt(prompt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdk_requires_api_key() {
        let config = AppConfig::default();
        // Default config has no API key.
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(SdkClient::new(config));
        assert!(result.is_err());
    }
}
