//! LSP server configuration.
//!
//! Describes how to launch a language server and which file extensions
//! it handles.  [`default_servers`] returns a set of well-known servers
//! (rust-analyzer, typescript-language-server, pyright) that can be used
//! as sensible defaults.

use serde::{Deserialize, Serialize};

/// Configuration for a single language server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    /// Human-readable name (e.g. "rust-analyzer").
    pub name: String,
    /// Command to launch the server.
    pub command: String,
    /// Additional CLI arguments.
    pub args: Vec<String>,
    /// File extensions this server handles (e.g. `["rs", "toml"]`).
    pub file_extensions: Vec<String>,
    /// Root URI to pass during `initialize`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_uri: Option<String>,
    /// Extra `initializationOptions` sent with the `initialize` request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initialization_options: Option<serde_json::Value>,
}

/// Returns the built-in default server configurations.
///
/// Currently includes:
/// * **rust-analyzer** -- Rust
/// * **typescript-language-server** -- TypeScript / JavaScript
/// * **pyright** -- Python
pub fn default_servers() -> Vec<LspServerConfig> {
    vec![
        LspServerConfig {
            name: "rust-analyzer".to_string(),
            command: "rust-analyzer".to_string(),
            args: vec![],
            file_extensions: vec!["rs".to_string(), "toml".to_string()],
            root_uri: None,
            initialization_options: None,
        },
        LspServerConfig {
            name: "typescript-language-server".to_string(),
            command: "typescript-language-server".to_string(),
            args: vec!["--stdio".to_string()],
            file_extensions: vec![
                "ts".to_string(),
                "tsx".to_string(),
                "js".to_string(),
                "jsx".to_string(),
            ],
            root_uri: None,
            initialization_options: None,
        },
        LspServerConfig {
            name: "pyright".to_string(),
            command: "pyright-langserver".to_string(),
            args: vec!["--stdio".to_string()],
            file_extensions: vec!["py".to_string(), "pyi".to_string()],
            root_uri: None,
            initialization_options: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_servers_not_empty() {
        let servers = default_servers();
        assert_eq!(servers.len(), 3);
        assert_eq!(servers[0].name, "rust-analyzer");
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let cfg = LspServerConfig {
            name: "test-server".to_string(),
            command: "/usr/bin/test-lsp".to_string(),
            args: vec!["--stdio".to_string()],
            file_extensions: vec!["txt".to_string()],
            root_uri: Some("file:///project".to_string()),
            initialization_options: Some(serde_json::json!({"foo": true})),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: LspServerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test-server");
        assert_eq!(back.root_uri.as_deref(), Some("file:///project"));
    }
}
