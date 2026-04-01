//! The main bridge client that ties all subsystems together.
//!
//! `BridgeClient` manages authentication, device trust, session management,
//! and message routing for the bridge connection to claude.ai.

use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use cc_error::{CcError, CcResult};

use crate::config::BridgeConfig;
use crate::device_trust::DeviceTrust;
use crate::jwt::{JwtToken, TokenRefresher};
use crate::messaging::{BridgeMessage, MessageQueue, MessageQueueReceiver};
use crate::session::{BridgeSession, SessionStatus};

/// The main bridge client for connecting to claude.ai.
pub struct BridgeClient {
    config: BridgeConfig,
    token: Option<JwtToken>,
    device: DeviceTrust,
    session: Option<BridgeSession>,
    message_queue: MessageQueue,
    queue_receiver: Option<MessageQueueReceiver>,
}

impl BridgeClient {
    /// Create a new `BridgeClient` with the given configuration.
    pub fn new(config: BridgeConfig) -> Self {
        let (queue, receiver) = MessageQueue::new(256);
        Self {
            config,
            token: None,
            device: DeviceTrust::new(),
            session: None,
            message_queue: queue,
            queue_receiver: Some(receiver),
        }
    }

    /// Authenticate with the bridge server.
    ///
    /// If a session token is available in the config, it is exchanged for
    /// a JWT token. Otherwise, an error is returned.
    pub async fn authenticate(&mut self) -> CcResult<()> {
        let session_token = self
            .config
            .session_token
            .as_deref()
            .ok_or_else(|| CcError::Auth("No session token configured for authentication".into()))?;

        info!("Authenticating with bridge server");

        let client = reqwest::Client::new();
        let url = format!("{}/auth/token", self.config.api_url);

        let resp = client
            .post(&url)
            .json(&serde_json::json!({
                "session_token": session_token,
                "device_id": self.device.device_id,
            }))
            .send()
            .await
            .map_err(|e| CcError::Api {
                message: format!("Authentication request failed: {e}"),
                status_code: None,
            })?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            return Err(CcError::Auth(format!(
                "Authentication failed with status {status}"
            )));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            CcError::Serialization(format!("Failed to parse auth response: {e}"))
        })?;

        let access_token = body["access_token"]
            .as_str()
            .ok_or_else(|| CcError::Auth("Missing access_token in auth response".into()))?
            .to_string();

        let refresh_token = body["refresh_token"].as_str().map(|s| s.to_string());
        let expires_in = body["expires_in"].as_i64().unwrap_or(3600);

        self.token = Some(JwtToken {
            access_token,
            refresh_token,
            expires_at: chrono::Utc::now() + chrono::Duration::seconds(expires_in),
        });

        info!("Bridge authentication successful");
        Ok(())
    }

    /// Connect to the bridge server over WebSocket.
    ///
    /// Requires a prior successful `authenticate()` call.
    pub async fn connect(&mut self) -> CcResult<()> {
        let token = self
            .token
            .as_ref()
            .ok_or_else(|| CcError::Auth("Not authenticated -- call authenticate() first".into()))?
            .clone();

        if token.is_expired() {
            return Err(CcError::Auth("Token has expired -- re-authenticate".into()));
        }

        let mut session = BridgeSession::new();
        session.connect(&self.config, &token).await?;
        self.session = Some(session);

        info!("Bridge client connected");
        Ok(())
    }

    /// Disconnect from the bridge server.
    pub async fn disconnect(&mut self) -> CcResult<()> {
        if let Some(mut session) = self.session.take() {
            session.disconnect().await?;
        }
        info!("Bridge client disconnected");
        Ok(())
    }

    /// Returns `true` if the bridge is connected.
    pub fn is_connected(&self) -> bool {
        self.session
            .as_ref()
            .map(|s| s.is_connected())
            .unwrap_or(false)
    }

    /// Send a message to the bridge server.
    pub async fn send(&mut self, msg: BridgeMessage) -> CcResult<()> {
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| CcError::Internal("Not connected".into()))?;
        session.send_message(msg).await
    }

    /// Receive the next message from the bridge server.
    pub async fn recv(&mut self) -> Option<BridgeMessage> {
        let session = self.session.as_mut()?;
        match session.recv_message().await {
            Some(Ok(msg)) => Some(msg),
            Some(Err(e)) => {
                error!("Error receiving message: {e}");
                None
            }
            None => None,
        }
    }

    /// Returns the current session ID, if connected.
    pub fn session_id(&self) -> Option<&str> {
        self.session.as_ref().map(|s| s.id.as_str())
    }

    /// Returns the message queue sender for enqueuing outbound messages.
    pub fn message_queue(&self) -> &MessageQueue {
        &self.message_queue
    }

    /// Take the queue receiver (can only be called once).
    pub fn take_queue_receiver(&mut self) -> Option<MessageQueueReceiver> {
        self.queue_receiver.take()
    }

    /// Run the main event loop, forwarding messages to the provided channel.
    ///
    /// This loop handles:
    /// - Receiving messages from the server and forwarding them
    /// - Processing queued outbound messages
    /// - Token refresh when nearing expiry
    /// - Automatic reconnection on connection loss
    pub async fn run_loop(
        &mut self,
        event_tx: mpsc::Sender<BridgeMessage>,
    ) -> CcResult<()> {
        info!("Starting bridge event loop");

        let refresher = TokenRefresher::new(self.config.clone());

        loop {
            // Check if token needs refreshing.
            if let Some(ref token) = self.token {
                if refresher.should_refresh(token) {
                    debug!("Proactively refreshing token");
                    match refresher.refresh(token).await {
                        Ok(new_token) => {
                            self.token = Some(new_token);
                        }
                        Err(e) => {
                            warn!("Token refresh failed: {e}");
                        }
                    }
                }
            }

            // Receive messages from the bridge.
            match self.recv().await {
                Some(BridgeMessage::Heartbeat) => {
                    debug!("Received heartbeat");
                }
                Some(msg) => {
                    if event_tx.send(msg).await.is_err() {
                        info!("Event receiver dropped, stopping loop");
                        break;
                    }
                }
                None => {
                    // Connection lost. Attempt reconnection.
                    warn!("Bridge connection lost, attempting reconnect");
                    if let Some(ref mut session) = self.session {
                        session.status = SessionStatus::Reconnecting;
                    }

                    // Wait before retrying.
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                    if let Err(e) = self.connect().await {
                        error!("Reconnection failed: {e}");
                        break;
                    }
                    info!("Reconnected to bridge");
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_client_is_disconnected() {
        let config = BridgeConfig::default_urls();
        let client = BridgeClient::new(config);
        assert!(!client.is_connected());
        assert!(client.session_id().is_none());
    }

    #[test]
    fn take_queue_receiver_returns_once() {
        let config = BridgeConfig::default_urls();
        let mut client = BridgeClient::new(config);
        assert!(client.take_queue_receiver().is_some());
        assert!(client.take_queue_receiver().is_none());
    }

    #[tokio::test]
    async fn authenticate_without_token_fails() {
        let config = BridgeConfig::default_urls();
        let mut client = BridgeClient::new(config);
        let result = client.authenticate().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn connect_without_auth_fails() {
        let config = BridgeConfig::default_urls();
        let mut client = BridgeClient::new(config);
        let result = client.connect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn disconnect_when_not_connected_is_ok() {
        let config = BridgeConfig::default_urls();
        let mut client = BridgeClient::new(config);
        assert!(client.disconnect().await.is_ok());
    }
}
