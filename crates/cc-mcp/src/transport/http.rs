//! HTTP streamable MCP transport.
//!
//! A simple transport that sends JSON-RPC requests as HTTP POST bodies
//! and reads the response from the HTTP response body. Suitable for
//! servers that support the MCP "Streamable HTTP" transport variant.

use async_trait::async_trait;
use cc_error::{CcError, CcResult};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::protocol::{JsonRpcNotification, JsonRpcRequest, RequestIdGenerator};

/// Communicates with an MCP server over plain HTTP POST requests.
///
/// Each request is an independent HTTP request/response cycle.
/// No persistent connection is maintained.
pub struct HttpStreamableTransport {
    url: String,
    client: reqwest::Client,
    id_gen: RequestIdGenerator,
    connected: AtomicBool,
}

impl HttpStreamableTransport {
    /// Create a new HTTP streamable transport targeting the given URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            client: reqwest::Client::new(),
            id_gen: RequestIdGenerator::new(),
            connected: AtomicBool::new(true),
        }
    }

    /// POST a JSON body and return the parsed JSON response.
    async fn post_json(&self, body: &serde_json::Value) -> CcResult<serde_json::Value> {
        let response = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| {
                self.connected.store(false, Ordering::Relaxed);
                CcError::Internal(format!("MCP HTTP request failed: {e}"))
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
            .map_err(|e| CcError::Serialization(format!("MCP HTTP response parse failed: {e}")))
    }
}

#[async_trait]
impl super::McpTransport for HttpStreamableTransport {
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

        // Best-effort for notifications.
        let _ = self.post_json(&body).await;
        Ok(())
    }

    async fn close(&self) -> CcResult<()> {
        self.connected.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}
