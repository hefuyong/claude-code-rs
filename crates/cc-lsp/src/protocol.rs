//! LSP protocol types.
//!
//! Lightweight representations of the JSON-RPC messages and common
//! structures used in the Language Server Protocol.  These are *not*
//! exhaustive -- they cover the subset that Claude Code RS needs for
//! diagnostics, completions, hover, and go-to-definition.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// JSON-RPC envelope types
// ---------------------------------------------------------------------------

/// A request sent to the language server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspRequest {
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

/// A response received from the language server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspResponse {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<LspError>,
}

/// A notification (no `id` -- fire-and-forget).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspNotification {
    pub method: String,
    pub params: serde_json::Value,
}

/// An error payload inside an [`LspResponse`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspError {
    pub code: i64,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Standard LSP method constants
// ---------------------------------------------------------------------------

pub const INITIALIZE: &str = "initialize";
pub const SHUTDOWN: &str = "shutdown";
pub const TEXT_DOCUMENT_DID_OPEN: &str = "textDocument/didOpen";
pub const TEXT_DOCUMENT_DID_CHANGE: &str = "textDocument/didChange";
pub const TEXT_DOCUMENT_DID_SAVE: &str = "textDocument/didSave";
pub const TEXT_DOCUMENT_DID_CLOSE: &str = "textDocument/didClose";
pub const TEXT_DOCUMENT_COMPLETION: &str = "textDocument/completion";
pub const TEXT_DOCUMENT_HOVER: &str = "textDocument/hover";
pub const TEXT_DOCUMENT_DEFINITION: &str = "textDocument/definition";
pub const TEXT_DOCUMENT_REFERENCES: &str = "textDocument/references";
pub const TEXT_DOCUMENT_DIAGNOSTICS: &str = "textDocument/publishDiagnostics";

// ---------------------------------------------------------------------------
// Common LSP data structures
// ---------------------------------------------------------------------------

/// A zero-based position inside a text document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

impl Position {
    pub fn new(line: u32, character: u32) -> Self {
        Self { line, character }
    }
}

/// A range within a text document expressed as start/end [`Position`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

/// A location in a document (URI + range).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

/// Diagnostic severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

impl DiagnosticSeverity {
    /// Convert from the LSP numeric severity (1=Error .. 4=Hint).
    pub fn from_lsp(value: u32) -> Self {
        match value {
            1 => Self::Error,
            2 => Self::Warning,
            3 => Self::Information,
            _ => Self::Hint,
        }
    }

    /// Convert to the LSP numeric severity.
    pub fn to_lsp(self) -> u32 {
        match self {
            Self::Error => 1,
            Self::Warning => 2,
            Self::Information => 3,
            Self::Hint => 4,
        }
    }
}

/// A single diagnostic (error, warning, etc.) reported by a language server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// A completion item returned by the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(rename = "insertText", skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,
}

// ---------------------------------------------------------------------------
// JSON-RPC wire helpers
// ---------------------------------------------------------------------------

/// Build a JSON-RPC 2.0 request object with Content-Length framing.
pub fn encode_request(id: u64, method: &str, params: serde_json::Value) -> String {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    let body_str = serde_json::to_string(&body).expect("serialize request");
    format!("Content-Length: {}\r\n\r\n{}", body_str.len(), body_str)
}

/// Build a JSON-RPC 2.0 notification (no `id`) with Content-Length framing.
pub fn encode_notification(method: &str, params: serde_json::Value) -> String {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    });
    let body_str = serde_json::to_string(&body).expect("serialize notification");
    format!("Content-Length: {}\r\n\r\n{}", body_str.len(), body_str)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_new() {
        let p = Position::new(10, 5);
        assert_eq!(p.line, 10);
        assert_eq!(p.character, 5);
    }

    #[test]
    fn test_diagnostic_severity_roundtrip() {
        for sev in [
            DiagnosticSeverity::Error,
            DiagnosticSeverity::Warning,
            DiagnosticSeverity::Information,
            DiagnosticSeverity::Hint,
        ] {
            assert_eq!(DiagnosticSeverity::from_lsp(sev.to_lsp()), sev);
        }
    }

    #[test]
    fn test_encode_request_has_content_length() {
        let msg = encode_request(1, INITIALIZE, serde_json::json!({}));
        assert!(msg.starts_with("Content-Length: "));
        assert!(msg.contains("\"method\":\"initialize\""));
    }

    #[test]
    fn test_encode_notification_has_no_id() {
        let msg = encode_notification(TEXT_DOCUMENT_DID_OPEN, serde_json::json!({}));
        assert!(!msg.contains("\"id\""));
        assert!(msg.contains("\"method\":\"textDocument/didOpen\""));
    }

    #[test]
    fn test_lsp_request_serde_roundtrip() {
        let req = LspRequest {
            id: 42,
            method: "textDocument/hover".to_string(),
            params: serde_json::json!({"uri": "file:///test.rs"}),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: LspRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, 42);
        assert_eq!(parsed.method, "textDocument/hover");
    }

    #[test]
    fn test_completion_item_serde() {
        let item = CompletionItem {
            label: "println!".to_string(),
            kind: Some(3),
            detail: Some("macro".to_string()),
            insert_text: Some("println!(\"$1\")".to_string()),
        };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"insertText\""));
        let back: CompletionItem = serde_json::from_str(&json).unwrap();
        assert_eq!(back.label, "println!");
    }
}
