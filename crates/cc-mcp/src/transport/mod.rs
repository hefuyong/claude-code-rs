//! Transport layer for MCP communication.
//!
//! Defines the [`McpTransport`] trait and provides three implementations:
//! - [`StdioTransport`] -- subprocess stdin/stdout
//! - [`SseTransport`] -- HTTP Server-Sent Events
//! - [`HttpStreamableTransport`] -- plain HTTP POST

pub mod http;
pub mod sse;
pub mod stdio;

pub use http::HttpStreamableTransport;
pub use sse::SseTransport;
pub use stdio::StdioTransport;

use async_trait::async_trait;
use cc_error::CcResult;

/// Abstraction over the wire transport to an MCP server.
///
/// All transports communicate using JSON-RPC 2.0 messages serialised as
/// `serde_json::Value`.  The transport is responsible for framing,
/// serialisation, and delivery -- callers deal only in `Value` objects.
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Send a JSON-RPC request and wait for the corresponding response.
    async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> CcResult<serde_json::Value>;

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> CcResult<()>;

    /// Cleanly shut down the transport.
    async fn close(&self) -> CcResult<()>;

    /// Returns `true` if the transport believes the connection is still alive.
    fn is_connected(&self) -> bool;
}
