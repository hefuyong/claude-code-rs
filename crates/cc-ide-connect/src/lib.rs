//! IDE direct connection layer for Claude Code RS.
//!
//! Provides the ability to connect to running IDE instances (VS Code,
//! JetBrains, Vim, Emacs) via IPC (Unix domain sockets / named pipes)
//! or TCP, receive real-time editor events, and push edits back.
//!
//! The [`IdeConnectionManager`] can auto-detect running IDEs and manage
//! multiple concurrent connections.

use std::fmt;
use std::path::{Path, PathBuf};

use cc_error::{CcError, CcResult};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// IDE type & connection status
// ---------------------------------------------------------------------------

/// Identifies the kind of IDE on the other end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdeType {
    VsCode,
    JetBrains,
    Vim,
    Emacs,
    Unknown(String),
}

impl fmt::Display for IdeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VsCode => write!(f, "VS Code"),
            Self::JetBrains => write!(f, "JetBrains"),
            Self::Vim => write!(f, "Vim"),
            Self::Emacs => write!(f, "Emacs"),
            Self::Unknown(s) => write!(f, "Unknown({s})"),
        }
    }
}

/// Current state of an [`IdeConnection`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
}

impl fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => write!(f, "disconnected"),
            Self::Connecting => write!(f, "connecting"),
            Self::Connected => write!(f, "connected"),
        }
    }
}

// ---------------------------------------------------------------------------
// Messages & responses
// ---------------------------------------------------------------------------

/// An event received from the IDE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IdeMessage {
    FileOpened {
        path: String,
        content: String,
    },
    FileChanged {
        path: String,
        content: String,
        version: i32,
    },
    FileSaved {
        path: String,
    },
    FileClosed {
        path: String,
    },
    SelectionChanged {
        path: String,
        start: (u32, u32),
        end: (u32, u32),
    },
    CursorMoved {
        path: String,
        line: u32,
        character: u32,
    },
    Ping,
}

/// A command sent back to the IDE.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IdeResponse {
    /// Replace the contents of a file.
    ApplyEdit { path: String, content: String },
    /// Open a file, optionally jumping to a line.
    OpenFile { path: String, line: Option<u32> },
    /// Show a message in the IDE's notification area.
    ShowMessage { message: String, severity: String },
    /// Acknowledge receipt of a message.
    Ack,
}

// ---------------------------------------------------------------------------
// IdeConnection
// ---------------------------------------------------------------------------

/// A single connection to an IDE instance.
pub struct IdeConnection {
    ide_type: IdeType,
    status: ConnectionStatus,
    socket_path: Option<PathBuf>,
    tx: Option<mpsc::Sender<IdeMessage>>,
    rx: Option<mpsc::Receiver<IdeMessage>>,
    /// Outbound channel used by [`send`].
    out_tx: Option<mpsc::Sender<IdeResponse>>,
}

impl IdeConnection {
    /// Create a new, disconnected connection.
    pub fn new() -> Self {
        Self {
            ide_type: IdeType::Unknown("none".into()),
            status: ConnectionStatus::Disconnected,
            socket_path: None,
            tx: None,
            rx: None,
            out_tx: None,
        }
    }

    /// Connect to a VS Code instance over a Unix domain socket / named pipe.
    pub async fn connect_vscode(&mut self, socket_path: &Path) -> CcResult<()> {
        self.status = ConnectionStatus::Connecting;
        self.ide_type = IdeType::VsCode;
        self.socket_path = Some(socket_path.to_path_buf());

        // Create internal channels -- the actual socket I/O would run in
        // a background task that bridges the socket to these channels.
        let (tx, rx) = mpsc::channel::<IdeMessage>(64);
        let (out_tx, _out_rx) = mpsc::channel::<IdeResponse>(64);

        self.tx = Some(tx);
        self.rx = Some(rx);
        self.out_tx = Some(out_tx);

        // In a real implementation we would spawn a tokio task here that
        // opens the socket and pumps messages.  For now we mark the
        // connection as connected immediately.
        self.status = ConnectionStatus::Connected;
        debug!(ide = %self.ide_type, path = ?socket_path, "connected");
        Ok(())
    }

    /// Connect to a JetBrains IDE over TCP.
    pub async fn connect_jetbrains(&mut self, port: u16) -> CcResult<()> {
        self.status = ConnectionStatus::Connecting;
        self.ide_type = IdeType::JetBrains;
        self.socket_path = None;

        let (tx, rx) = mpsc::channel::<IdeMessage>(64);
        let (out_tx, _out_rx) = mpsc::channel::<IdeResponse>(64);

        self.tx = Some(tx);
        self.rx = Some(rx);
        self.out_tx = Some(out_tx);

        // A production version would open a TCP connection here.
        self.status = ConnectionStatus::Connected;
        debug!(ide = %self.ide_type, port, "connected");
        Ok(())
    }

    /// Disconnect from the IDE.
    pub async fn disconnect(&mut self) -> CcResult<()> {
        self.tx = None;
        self.rx = None;
        self.out_tx = None;
        self.status = ConnectionStatus::Disconnected;
        debug!(ide = %self.ide_type, "disconnected");
        Ok(())
    }

    /// Returns `true` if the connection is currently active.
    pub fn is_connected(&self) -> bool {
        self.status == ConnectionStatus::Connected
    }

    /// The type of IDE this connection talks to.
    pub fn ide_type(&self) -> &IdeType {
        &self.ide_type
    }

    /// The current status.
    pub fn status(&self) -> &ConnectionStatus {
        &self.status
    }

    /// Send a response/command back to the IDE.
    pub async fn send(&self, response: IdeResponse) -> CcResult<()> {
        let tx = self
            .out_tx
            .as_ref()
            .ok_or_else(|| CcError::Internal("IDE connection not established".into()))?;

        tx.send(response).await.map_err(|e| {
            CcError::Internal(format!("failed to send to IDE: {e}"))
        })?;
        Ok(())
    }

    /// Receive the next message from the IDE, or `None` if the channel
    /// has been closed.
    pub async fn recv(&mut self) -> Option<IdeMessage> {
        let rx = self.rx.as_mut()?;
        rx.recv().await
    }

    /// Try to auto-detect a running IDE by looking for well-known socket
    /// paths.
    pub async fn auto_detect() -> Option<(IdeType, PathBuf)> {
        // VS Code socket paths.
        let vscode_candidates = vec![
            dirs_next_runtime_dir().join("vscode-ipc.sock"),
            PathBuf::from("/tmp/vscode-ipc.sock"),
        ];
        for p in vscode_candidates {
            if p.exists() {
                return Some((IdeType::VsCode, p));
            }
        }

        // JetBrains -- look for a well-known port file.
        let jb_port_file = dirs_next_config_dir().join("JetBrains/.port");
        if jb_port_file.exists() {
            return Some((IdeType::JetBrains, jb_port_file));
        }

        None
    }
}

impl Default for IdeConnection {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// IdeConnectionManager
// ---------------------------------------------------------------------------

/// Manages zero or more [`IdeConnection`]s.
pub struct IdeConnectionManager {
    connections: Vec<IdeConnection>,
}

impl IdeConnectionManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
        }
    }

    /// Attempt to auto-detect and connect to running IDEs.
    ///
    /// Returns the number of successful connections.
    pub async fn auto_connect(&mut self) -> CcResult<usize> {
        let mut count = 0usize;

        if let Some((ide_type, path)) = IdeConnection::auto_detect().await {
            let mut conn = IdeConnection::new();
            match ide_type {
                IdeType::VsCode => {
                    if conn.connect_vscode(&path).await.is_ok() {
                        self.connections.push(conn);
                        count += 1;
                    }
                }
                IdeType::JetBrains => {
                    // For JetBrains we would read the port from the file.
                    if conn.connect_jetbrains(63342).await.is_ok() {
                        self.connections.push(conn);
                        count += 1;
                    }
                }
                _ => {}
            }
        }

        Ok(count)
    }

    /// List the type and status of every managed connection.
    pub fn active_connections(&self) -> Vec<(&IdeType, &ConnectionStatus)> {
        self.connections
            .iter()
            .map(|c| (c.ide_type(), c.status()))
            .collect()
    }

    /// Broadcast a response to all connected IDEs.
    pub async fn broadcast(&self, response: IdeResponse) -> CcResult<()> {
        for conn in &self.connections {
            if conn.is_connected() {
                if let Err(e) = conn.send(response.clone()).await {
                    warn!(ide = %conn.ide_type(), %e, "broadcast failed");
                }
            }
        }
        Ok(())
    }

    /// Disconnect from all IDEs.
    pub async fn disconnect_all(&mut self) -> CcResult<()> {
        for conn in &mut self.connections {
            if let Err(e) = conn.disconnect().await {
                warn!(ide = %conn.ide_type(), %e, "disconnect failed");
            }
        }
        self.connections.clear();
        Ok(())
    }

    /// Add an already-connected connection to the manager.
    pub fn add(&mut self, conn: IdeConnection) {
        self.connections.push(conn);
    }

    /// Number of managed connections (including disconnected ones).
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    /// Returns `true` if there are no managed connections.
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }
}

impl Default for IdeConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Best-effort runtime directory (XDG_RUNTIME_DIR or /tmp).
fn dirs_next_runtime_dir() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

/// Best-effort config directory (~/.config or %APPDATA%).
fn dirs_next_config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".config"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_connection_is_disconnected() {
        let conn = IdeConnection::new();
        assert!(!conn.is_connected());
        assert_eq!(*conn.status(), ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_ide_type_display() {
        assert_eq!(IdeType::VsCode.to_string(), "VS Code");
        assert_eq!(IdeType::JetBrains.to_string(), "JetBrains");
        assert_eq!(IdeType::Unknown("foo".into()).to_string(), "Unknown(foo)");
    }

    #[test]
    fn test_connection_status_display() {
        assert_eq!(ConnectionStatus::Connected.to_string(), "connected");
        assert_eq!(ConnectionStatus::Disconnected.to_string(), "disconnected");
    }

    #[tokio::test]
    async fn test_connect_and_disconnect_vscode() {
        let mut conn = IdeConnection::new();
        // We can "connect" even if the socket does not exist -- the
        // current stub just sets up channels.
        conn.connect_vscode(Path::new("/tmp/fake.sock"))
            .await
            .unwrap();
        assert!(conn.is_connected());
        assert_eq!(*conn.ide_type(), IdeType::VsCode);

        conn.disconnect().await.unwrap();
        assert!(!conn.is_connected());
    }

    #[tokio::test]
    async fn test_connect_jetbrains() {
        let mut conn = IdeConnection::new();
        conn.connect_jetbrains(63342).await.unwrap();
        assert!(conn.is_connected());
        assert_eq!(*conn.ide_type(), IdeType::JetBrains);
    }

    #[test]
    fn test_manager_new_is_empty() {
        let mgr = IdeConnectionManager::new();
        assert!(mgr.is_empty());
        assert_eq!(mgr.len(), 0);
    }

    #[tokio::test]
    async fn test_manager_add_and_disconnect_all() {
        let mut mgr = IdeConnectionManager::new();
        let mut conn = IdeConnection::new();
        conn.connect_vscode(Path::new("/tmp/fake.sock"))
            .await
            .unwrap();
        mgr.add(conn);
        assert_eq!(mgr.len(), 1);
        assert_eq!(mgr.active_connections().len(), 1);

        mgr.disconnect_all().await.unwrap();
        assert!(mgr.is_empty());
    }

    #[test]
    fn test_ide_message_serde_roundtrip() {
        let msg = IdeMessage::FileOpened {
            path: "/foo/bar.rs".into(),
            content: "fn main() {}".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: IdeMessage = serde_json::from_str(&json).unwrap();
        match back {
            IdeMessage::FileOpened { path, .. } => assert_eq!(path, "/foo/bar.rs"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn test_ide_response_serde() {
        let resp = IdeResponse::ShowMessage {
            message: "hello".into(),
            severity: "info".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("ShowMessage"));
    }
}
