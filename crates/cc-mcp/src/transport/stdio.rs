//! Stdio-based MCP transport.
//!
//! Spawns a child process and communicates with it via newline-delimited
//! JSON over stdin/stdout.

use async_trait::async_trait;
use cc_error::{CcError, CcResult};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use crate::protocol::{JsonRpcNotification, JsonRpcRequest, RequestIdGenerator};

/// Communicates with an MCP server over stdin/stdout of a child process.
///
/// Each JSON-RPC message is sent as a single line of JSON followed by `\n`.
/// Responses are read one line at a time from stdout.
pub struct StdioTransport {
    stdin: Mutex<tokio::process::ChildStdin>,
    stdout: Mutex<BufReader<tokio::process::ChildStdout>>,
    child: Mutex<tokio::process::Child>,
    id_gen: RequestIdGenerator,
    connected: AtomicBool,
}

impl StdioTransport {
    /// Spawn a child process and create a transport connected to it.
    pub async fn spawn(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> CcResult<Self> {
        let mut cmd = tokio::process::Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        for (key, value) in env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().map_err(|e| {
            CcError::Internal(format!("failed to spawn MCP server '{command}': {e}"))
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| CcError::Internal("MCP child process has no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CcError::Internal("MCP child process has no stdout".into()))?;

        Ok(Self {
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            child: Mutex::new(child),
            id_gen: RequestIdGenerator::new(),
            connected: AtomicBool::new(true),
        })
    }

    /// Write a serialised JSON line to stdin and read a response line from stdout.
    async fn send_line(&self, line: &str) -> CcResult<String> {
        let mut payload = line.to_string();
        payload.push('\n');

        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| CcError::Internal(format!("MCP stdin write failed: {e}")))?;
        stdin
            .flush()
            .await
            .map_err(|e| CcError::Internal(format!("MCP stdin flush failed: {e}")))?;
        drop(stdin);

        let mut response_line = String::new();
        let mut stdout = self.stdout.lock().await;
        let bytes_read = stdout
            .read_line(&mut response_line)
            .await
            .map_err(|e| CcError::Internal(format!("MCP stdout read failed: {e}")))?;

        if bytes_read == 0 {
            self.connected.store(false, Ordering::Relaxed);
            return Err(CcError::Internal(
                "MCP server closed stdout unexpectedly".into(),
            ));
        }

        Ok(response_line)
    }
}

#[async_trait]
impl super::McpTransport for StdioTransport {
    async fn send_request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> CcResult<serde_json::Value> {
        let request = JsonRpcRequest::new(self.id_gen.next(), method, params);
        let line = serde_json::to_string(&request)
            .map_err(|e| CcError::Serialization(e.to_string()))?;

        let response_line = self.send_line(&line).await?;

        serde_json::from_str(&response_line)
            .map_err(|e| CcError::Serialization(format!("MCP response parse failed: {e}")))
    }

    async fn send_notification(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> CcResult<()> {
        let notification = JsonRpcNotification::new(method, params);
        let line = serde_json::to_string(&notification)
            .map_err(|e| CcError::Serialization(e.to_string()))?;

        let mut payload = line;
        payload.push('\n');

        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(payload.as_bytes())
            .await
            .map_err(|e| CcError::Internal(format!("MCP stdin write failed: {e}")))?;
        stdin
            .flush()
            .await
            .map_err(|e| CcError::Internal(format!("MCP stdin flush failed: {e}")))?;

        Ok(())
    }

    async fn close(&self) -> CcResult<()> {
        self.connected.store(false, Ordering::Relaxed);
        let mut child = self.child.lock().await;
        let _ = child.kill().await;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}
