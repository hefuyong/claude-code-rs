//! Bridge session management.
//!
//! Manages the lifecycle of a WebSocket session to the bridge server,
//! including connecting, disconnecting, and sending messages.

use chrono::{DateTime, Utc};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info};
use uuid::Uuid;

use cc_error::{CcError, CcResult};

use crate::config::BridgeConfig;
use crate::jwt::JwtToken;
use crate::messaging::BridgeMessage;

/// The status of a bridge session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionStatus {
    /// Attempting to establish the connection.
    Connecting,
    /// Connection is active and ready.
    Connected,
    /// Connection was lost and is being re-established.
    Reconnecting,
    /// Connection is closed.
    Disconnected,
}

/// Internal wrapper around the WebSocket connection halves.
struct WebSocketConnection {
    sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, WsMessage>,
    stream: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

/// A session with the bridge server.
pub struct BridgeSession {
    /// Unique session identifier.
    pub id: String,
    /// Current session status.
    pub status: SessionStatus,
    /// When the session was created.
    pub started_at: DateTime<Utc>,
    /// The underlying WebSocket connection, if established.
    ws_connection: Option<WebSocketConnection>,
}

impl BridgeSession {
    /// Create a new disconnected session.
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            status: SessionStatus::Disconnected,
            started_at: Utc::now(),
            ws_connection: None,
        }
    }

    /// Connect to the bridge server over WebSocket.
    pub async fn connect(&mut self, config: &BridgeConfig, token: &JwtToken) -> CcResult<()> {
        if self.status == SessionStatus::Connected {
            return Ok(());
        }

        self.status = SessionStatus::Connecting;
        info!(session_id = %self.id, "Connecting to bridge at {}", config.ws_url);

        let url = format!(
            "{}?session_id={}&token={}",
            config.ws_url, self.id, token.access_token
        );

        let (ws_stream, _response) = connect_async(&url).await.map_err(|e| {
            self.status = SessionStatus::Disconnected;
            CcError::Api {
                message: format!("WebSocket connection failed: {e}"),
                status_code: None,
            }
        })?;

        let (sink, stream) = ws_stream.split();
        self.ws_connection = Some(WebSocketConnection { sink, stream });
        self.status = SessionStatus::Connected;

        debug!(session_id = %self.id, "Bridge session connected");
        Ok(())
    }

    /// Disconnect the session, closing the WebSocket.
    pub async fn disconnect(&mut self) -> CcResult<()> {
        if let Some(mut conn) = self.ws_connection.take() {
            let _ = conn.sink.close().await;
            info!(session_id = %self.id, "Bridge session disconnected");
        }
        self.status = SessionStatus::Disconnected;
        Ok(())
    }

    /// Send a bridge message over the WebSocket.
    pub async fn send_message(&mut self, msg: BridgeMessage) -> CcResult<()> {
        let conn = self.ws_connection.as_mut().ok_or_else(|| {
            CcError::Internal("Cannot send message: not connected".into())
        })?;

        let payload = msg.serialize()?;
        conn.sink
            .send(WsMessage::Text(payload.into()))
            .await
            .map_err(|e| CcError::Api {
                message: format!("Failed to send WebSocket message: {e}"),
                status_code: None,
            })
    }

    /// Receive the next bridge message from the WebSocket.
    ///
    /// Returns `None` if the connection has been closed.
    pub async fn recv_message(&mut self) -> Option<CcResult<BridgeMessage>> {
        let conn = self.ws_connection.as_mut()?;

        match conn.stream.next().await {
            Some(Ok(WsMessage::Text(text))) => {
                Some(BridgeMessage::deserialize(&text))
            }
            Some(Ok(WsMessage::Ping(_))) => {
                debug!("Received WebSocket ping");
                Some(Ok(BridgeMessage::Heartbeat))
            }
            Some(Ok(WsMessage::Close(_))) => {
                info!(session_id = %self.id, "WebSocket closed by server");
                None
            }
            Some(Err(e)) => {
                error!(session_id = %self.id, "WebSocket error: {e}");
                Some(Err(CcError::Api {
                    message: format!("WebSocket receive error: {e}"),
                    status_code: None,
                }))
            }
            _ => None,
        }
    }

    /// Returns the current session status.
    pub fn status(&self) -> &SessionStatus {
        &self.status
    }

    /// Returns `true` if the session is currently connected.
    pub fn is_connected(&self) -> bool {
        self.status == SessionStatus::Connected && self.ws_connection.is_some()
    }
}

impl Default for BridgeSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_is_disconnected() {
        let session = BridgeSession::new();
        assert_eq!(*session.status(), SessionStatus::Disconnected);
        assert!(!session.is_connected());
        assert!(!session.id.is_empty());
    }

    #[test]
    fn session_has_unique_id() {
        let s1 = BridgeSession::new();
        let s2 = BridgeSession::new();
        assert_ne!(s1.id, s2.id);
    }

    #[tokio::test]
    async fn disconnect_on_already_disconnected_is_ok() {
        let mut session = BridgeSession::new();
        assert!(session.disconnect().await.is_ok());
    }

    #[test]
    fn test_session_status_transitions() {
        let mut session = BridgeSession::new();

        // Starts as Disconnected.
        assert_eq!(session.status, SessionStatus::Disconnected);
        assert!(!session.is_connected());

        // Simulate Connecting transition.
        session.status = SessionStatus::Connecting;
        assert_eq!(*session.status(), SessionStatus::Connecting);
        assert!(!session.is_connected()); // no ws_connection, so still false

        // Simulate Connected transition.
        session.status = SessionStatus::Connected;
        assert_eq!(*session.status(), SessionStatus::Connected);
        // is_connected() requires both status == Connected AND ws_connection.is_some(),
        // so without an actual WebSocket it should still be false.
        assert!(!session.is_connected());

        // Simulate Reconnecting transition.
        session.status = SessionStatus::Reconnecting;
        assert_eq!(*session.status(), SessionStatus::Reconnecting);
        assert!(!session.is_connected());

        // Back to Disconnected.
        session.status = SessionStatus::Disconnected;
        assert_eq!(*session.status(), SessionStatus::Disconnected);
        assert!(!session.is_connected());
    }
}
