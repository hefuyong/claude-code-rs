//! SSE-based MCP transport.
//!
//! Connects to an MCP server over HTTP Server-Sent Events.
//! Requests are sent via POST; the server may push events over
//! a long-lived GET connection.

use async_trait::async_trait;
use cc_error::{CcError, CcResult};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::protocol::{JsonRpcNotification, JsonRpcRequest, RequestIdGenerator};

/// Communicates with an MCP server using HTTP Server-Sent Events.
///
/// Requests are POSTed as JSON-RPC to the server URL.
/// Responses are returned directly from the POST response body.
pub struct SseTransport {
    url: String,
    client: reqwest::Client,
    id_gen: RequestIdGenerator,
    connected: AtomicBool,
}

impl SseTransport {
    /// Create a new SSE transport targeting the given URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client: reqwest::Client::new(),
            id_gen: RequestIdGenerator::new(),
            connected: AtomicBool::new(true),
        }
    }

    /// Send a JSON body via POST and return the parsed response.
    async fn post_json(&self, body: &serde_json::Value) -> CcResult<serde_json::Value> {
        let response = self
            .client
            .post(&self.url)
            .json(body)
            .send()
            .await
            .map_err(|e| {
                self.connected.store(false, Ordering::Relaxed);
                CcError::Internal(format!("MCP SSE request failed: {e}"))
            })?;

        let status = response.status().as_u16();
        if status != 200 {
            let body_text = response.text().await.unwrap_or_default();
            return Err(CcError::Api {
                message: format!("MCP server returned {status}: {body_text}"),
                status_code: Some(status),
            });
        }

        response
            .json()
            .await
            .map_err(|e| CcError::Serialization(format!("MCP SSE response parse failed: {e}")))
    }
}

#[async_trait]
impl super::McpTransport for SseTransport {
    async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> CcResult<serde_json::Value> {
        let request = JsonRpcRequest::new(self.id_gen.next(), method, params);
        let body = serde_json::to_value(&request)
            .map_err(|e| CcError::Serialization(e.to_string()))?;

        self.post_json(&body).await
    }

    async fn send_notification(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> CcResult<()> {
        let notification = JsonRpcNotification::new(method, params);
        let body = serde_json::to_value(&notification)
            .map_err(|e| CcError::Serialization(e.to_string()))?;

        // Best-effort: notifications don't require a response.
        let _ = self.post_json(&body).await;
        Ok(())
    }

    async fn close(&self) -> CcResult<()> {
        self.connected.store(false, Ordering::Relaxed);
        // HTTP is stateless; nothing to close.
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}
