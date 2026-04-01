//! Single LSP server instance.
//!
//! [`LspServer`] wraps a child process that speaks the LSP stdio protocol.
//! It handles framing (Content-Length headers), serialization, and exposes
//! typed convenience methods for the most common LSP operations.

use std::path::Path;

use cc_error::{CcError, CcResult};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tracing::{debug, warn};

use crate::config::LspServerConfig;
use crate::protocol::*;

/// Status of a language server process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerStatus {
    Starting,
    Running,
    Stopped,
    Failed(String),
}

/// A single LSP server instance.
pub struct LspServer {
    /// Configuration that was used to start this server.
    pub config: LspServerConfig,
    /// Current lifecycle status.
    pub status: ServerStatus,
    /// The child process handle.
    child: Option<Child>,
    /// Buffered writer to the child's stdin.
    stdin: Option<BufWriter<ChildStdin>>,
    /// Buffered reader from the child's stdout.
    stdout: Option<BufReader<ChildStdout>>,
    /// Monotonically increasing request id.
    next_id: u64,
}

impl LspServer {
    /// Create a new server instance in the `Stopped` state.
    pub fn new(config: LspServerConfig) -> Self {
        Self {
            config,
            status: ServerStatus::Stopped,
            child: None,
            stdin: None,
            stdout: None,
            next_id: 1,
        }
    }

    /// Spawn the language server process and send `initialize`.
    pub async fn start(&mut self, workspace_root: &Path) -> CcResult<()> {
        self.status = ServerStatus::Starting;

        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());

        let mut child = cmd.spawn().map_err(|e| {
            let msg = format!("failed to start {}: {e}", self.config.command);
            self.status = ServerStatus::Failed(msg.clone());
            CcError::Internal(msg)
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| CcError::Internal("child stdin unavailable".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CcError::Internal("child stdout unavailable".into()))?;

        self.stdin = Some(BufWriter::new(stdin));
        self.stdout = Some(BufReader::new(stdout));
        self.child = Some(child);

        // Build the root URI.
        let root_uri = self
            .config
            .root_uri
            .clone()
            .unwrap_or_else(|| format!("file://{}", workspace_root.display()));

        let init_params = serde_json::json!({
            "processId": std::process::id(),
            "rootUri": root_uri,
            "capabilities": {},
            "initializationOptions": self.config.initialization_options,
        });

        let _resp = self.send_request(INITIALIZE, init_params).await?;

        // Send the mandatory `initialized` notification.
        self.send_notification("initialized", serde_json::json!({}))
            .await?;

        self.status = ServerStatus::Running;
        debug!(server = %self.config.name, "LSP server started");
        Ok(())
    }

    /// Send `shutdown` + `exit` to the server and wait for it to terminate.
    pub async fn shutdown(&mut self) -> CcResult<()> {
        if self.status != ServerStatus::Running {
            return Ok(());
        }

        // shutdown is a request -- we expect a response.
        if let Err(e) = self.send_request(SHUTDOWN, serde_json::Value::Null).await {
            warn!(server = %self.config.name, %e, "shutdown request failed");
        }

        // exit is a notification.
        if let Err(e) = self.send_notification("exit", serde_json::Value::Null).await {
            warn!(server = %self.config.name, %e, "exit notification failed");
        }

        // Wait briefly for the child to exit.
        if let Some(ref mut child) = self.child {
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                child.wait(),
            )
            .await;
        }

        self.status = ServerStatus::Stopped;
        self.stdin = None;
        self.stdout = None;
        self.child = None;
        debug!(server = %self.config.name, "LSP server stopped");
        Ok(())
    }

    // ------------------------------------------------------------------
    // Low-level transport
    // ------------------------------------------------------------------

    /// Send a JSON-RPC request and wait for the response.
    pub async fn send_request(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> CcResult<serde_json::Value> {
        let id = self.next_id;
        self.next_id += 1;

        let frame = encode_request(id, method, params);
        self.write_frame(&frame).await?;

        // Read back the response.
        let body = self.read_frame().await?;
        let resp: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| CcError::Serialization(format!("bad LSP response: {e}")))?;

        if let Some(err) = resp.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown LSP error");
            return Err(CcError::Internal(format!("LSP error ({method}): {msg}")));
        }

        Ok(resp.get("result").cloned().unwrap_or(serde_json::Value::Null))
    }

    /// Send a JSON-RPC notification (fire-and-forget).
    pub async fn send_notification(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> CcResult<()> {
        let frame = encode_notification(method, params);
        self.write_frame(&frame).await
    }

    // ------------------------------------------------------------------
    // Convenience methods
    // ------------------------------------------------------------------

    /// Notify the server that a document was opened.
    pub async fn notify_did_open(
        &mut self,
        uri: &str,
        language: &str,
        text: &str,
    ) -> CcResult<()> {
        self.send_notification(
            TEXT_DOCUMENT_DID_OPEN,
            serde_json::json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language,
                    "version": 1,
                    "text": text,
                }
            }),
        )
        .await
    }

    /// Notify the server that a document was changed.
    pub async fn notify_did_change(
        &mut self,
        uri: &str,
        text: &str,
        version: i32,
    ) -> CcResult<()> {
        self.send_notification(
            TEXT_DOCUMENT_DID_CHANGE,
            serde_json::json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }],
            }),
        )
        .await
    }

    /// Notify the server that a document was saved.
    pub async fn notify_did_save(&mut self, uri: &str) -> CcResult<()> {
        self.send_notification(
            TEXT_DOCUMENT_DID_SAVE,
            serde_json::json!({ "textDocument": { "uri": uri } }),
        )
        .await
    }

    /// Notify the server that a document was closed.
    pub async fn notify_did_close(&mut self, uri: &str) -> CcResult<()> {
        self.send_notification(
            TEXT_DOCUMENT_DID_CLOSE,
            serde_json::json!({ "textDocument": { "uri": uri } }),
        )
        .await
    }

    /// Request completions at the given position.
    pub async fn get_completions(
        &mut self,
        uri: &str,
        position: Position,
    ) -> CcResult<Vec<CompletionItem>> {
        let result = self
            .send_request(
                TEXT_DOCUMENT_COMPLETION,
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                }),
            )
            .await?;

        // The result may be a CompletionList or an array of CompletionItem.
        let items = if let Some(arr) = result.as_array() {
            arr.clone()
        } else if let Some(arr) = result.get("items").and_then(|v| v.as_array()) {
            arr.clone()
        } else {
            vec![]
        };

        items
            .into_iter()
            .map(|v| {
                serde_json::from_value(v)
                    .map_err(|e| CcError::Serialization(format!("bad completion item: {e}")))
            })
            .collect()
    }

    /// Request hover information at the given position.
    pub async fn get_hover(
        &mut self,
        uri: &str,
        position: Position,
    ) -> CcResult<Option<String>> {
        let result = self
            .send_request(
                TEXT_DOCUMENT_HOVER,
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                }),
            )
            .await?;

        if result.is_null() {
            return Ok(None);
        }

        // `contents` can be a string, MarkupContent, or MarkedString array.
        let contents = result.get("contents");
        let text = match contents {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(obj) if obj.get("value").is_some() => {
                obj["value"].as_str().unwrap_or("").to_string()
            }
            _ => serde_json::to_string_pretty(&result).unwrap_or_default(),
        };

        Ok(Some(text))
    }

    /// Request go-to-definition at the given position.
    pub async fn get_definition(
        &mut self,
        uri: &str,
        position: Position,
    ) -> CcResult<Vec<Location>> {
        let result = self
            .send_request(
                TEXT_DOCUMENT_DEFINITION,
                serde_json::json!({
                    "textDocument": { "uri": uri },
                    "position": position,
                }),
            )
            .await?;

        // Result may be a single Location, an array of Locations, or null.
        if result.is_null() {
            return Ok(vec![]);
        }

        if result.is_array() {
            let locs: Vec<Location> = serde_json::from_value(result)
                .map_err(|e| CcError::Serialization(format!("bad definition: {e}")))?;
            Ok(locs)
        } else {
            let loc: Location = serde_json::from_value(result)
                .map_err(|e| CcError::Serialization(format!("bad definition: {e}")))?;
            Ok(vec![loc])
        }
    }

    // ------------------------------------------------------------------
    // Internal I/O helpers
    // ------------------------------------------------------------------

    async fn write_frame(&mut self, frame: &str) -> CcResult<()> {
        let writer = self
            .stdin
            .as_mut()
            .ok_or_else(|| CcError::Internal("server stdin not available".into()))?;
        writer.write_all(frame.as_bytes()).await?;
        writer.flush().await?;
        Ok(())
    }

    async fn read_frame(&mut self) -> CcResult<String> {
        let reader = self
            .stdout
            .as_mut()
            .ok_or_else(|| CcError::Internal("server stdout not available".into()))?;

        // Read headers until we find Content-Length and then a blank line.
        let mut content_length: Option<usize> = None;
        loop {
            let mut header_line = String::new();
            let n = reader.read_line(&mut header_line).await?;
            if n == 0 {
                return Err(CcError::Internal("unexpected EOF from LSP server".into()));
            }

            let trimmed = header_line.trim();
            if trimmed.is_empty() {
                break;
            }

            if let Some(val) = trimmed.strip_prefix("Content-Length:") {
                content_length = Some(
                    val.trim()
                        .parse::<usize>()
                        .map_err(|e| CcError::Serialization(format!("bad Content-Length: {e}")))?,
                );
            }
        }

        let length = content_length
            .ok_or_else(|| CcError::Serialization("missing Content-Length header".into()))?;

        let mut body = vec![0u8; length];
        reader.read_exact(&mut body).await?;

        String::from_utf8(body)
            .map_err(|e| CcError::Serialization(format!("invalid UTF-8 in LSP body: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LspServerConfig;

    fn test_config() -> LspServerConfig {
        LspServerConfig {
            name: "test".to_string(),
            command: "echo".to_string(),
            args: vec![],
            file_extensions: vec!["rs".to_string()],
            root_uri: None,
            initialization_options: None,
        }
    }

    #[test]
    fn test_new_server_is_stopped() {
        let server = LspServer::new(test_config());
        assert_eq!(server.status, ServerStatus::Stopped);
        assert_eq!(server.next_id, 1);
    }

    #[test]
    fn test_server_config_preserved() {
        let cfg = test_config();
        let server = LspServer::new(cfg);
        assert_eq!(server.config.name, "test");
    }
}
