//! Multi-server manager.
//!
//! [`LspManager`] owns zero or more [`LspServer`] instances and routes
//! file-level events (open, change, save, close) to the appropriate
//! server based on file extension.

use std::collections::HashMap;
use std::path::Path;

use cc_error::{CcError, CcResult};
use tracing::{debug, warn};

use crate::config::LspServerConfig;
use crate::server::{LspServer, ServerStatus};

/// Manages multiple language servers and routes file events.
pub struct LspManager {
    /// Servers keyed by their name.
    servers: HashMap<String, LspServer>,
    /// Maps a file extension to a server name.
    extension_map: HashMap<String, String>,
}

impl LspManager {
    /// Create an empty manager.
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            extension_map: HashMap::new(),
        }
    }

    /// Start a language server and register it.
    pub async fn start_server(
        &mut self,
        config: LspServerConfig,
        workspace: &Path,
    ) -> CcResult<()> {
        let name = config.name.clone();

        // Register extension mappings.
        for ext in &config.file_extensions {
            self.extension_map.insert(ext.clone(), name.clone());
        }

        let mut server = LspServer::new(config);
        server.start(workspace).await?;

        self.servers.insert(name.clone(), server);
        debug!(server = %name, "registered language server");
        Ok(())
    }

    /// Stop a specific server by name.
    pub async fn stop_server(&mut self, name: &str) -> CcResult<()> {
        let server = self
            .servers
            .get_mut(name)
            .ok_or_else(|| CcError::NotFound(format!("no server named '{name}'")))?;

        server.shutdown().await?;

        // Remove extension mappings that pointed to this server.
        self.extension_map.retain(|_, v| v != name);
        self.servers.remove(name);
        Ok(())
    }

    /// Stop all registered servers.
    pub async fn stop_all(&mut self) -> CcResult<()> {
        let names: Vec<String> = self.servers.keys().cloned().collect();
        for name in names {
            if let Err(e) = self.stop_server(&name).await {
                warn!(server = %name, %e, "failed to stop server");
            }
        }
        Ok(())
    }

    /// Return the server responsible for a given file path, if any.
    ///
    /// The lookup is based on the file extension.
    pub fn get_server_for_file(&mut self, file_path: &str) -> Option<&mut LspServer> {
        let ext = Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string())?;

        let server_name = self.extension_map.get(&ext)?.clone();
        self.servers.get_mut(&server_name)
    }

    /// List all servers and their current status.
    pub fn list_servers(&self) -> Vec<(&str, &ServerStatus)> {
        self.servers
            .iter()
            .map(|(name, server)| (name.as_str(), &server.status))
            .collect()
    }

    // ------------------------------------------------------------------
    // File event routing
    // ------------------------------------------------------------------

    /// Notify the appropriate server that a file was opened.
    pub async fn notify_file_opened(&mut self, path: &str, content: &str) -> CcResult<()> {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let language = extension_to_language(ext);
        let uri = path_to_uri(path);

        if let Some(server) = self.get_server_for_file(path) {
            server.notify_did_open(&uri, language, content).await?;
        }
        Ok(())
    }

    /// Notify the appropriate server that a file was changed.
    pub async fn notify_file_changed(&mut self, path: &str, content: &str) -> CcResult<()> {
        let uri = path_to_uri(path);
        if let Some(server) = self.get_server_for_file(path) {
            server.notify_did_change(&uri, content, 0).await?;
        }
        Ok(())
    }

    /// Notify the appropriate server that a file was saved.
    pub async fn notify_file_saved(&mut self, path: &str) -> CcResult<()> {
        let uri = path_to_uri(path);
        if let Some(server) = self.get_server_for_file(path) {
            server.notify_did_save(&uri).await?;
        }
        Ok(())
    }

    /// Notify the appropriate server that a file was closed.
    pub async fn notify_file_closed(&mut self, path: &str) -> CcResult<()> {
        let uri = path_to_uri(path);
        if let Some(server) = self.get_server_for_file(path) {
            server.notify_did_close(&uri).await?;
        }
        Ok(())
    }
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a file extension to an LSP language identifier.
fn extension_to_language(ext: &str) -> &str {
    match ext {
        "rs" => "rust",
        "ts" => "typescript",
        "tsx" => "typescriptreact",
        "js" => "javascript",
        "jsx" => "javascriptreact",
        "py" | "pyi" => "python",
        "toml" => "toml",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "md" => "markdown",
        _ => ext,
    }
}

/// Convert a filesystem path to a `file://` URI.
fn path_to_uri(path: &str) -> String {
    if path.starts_with("file://") {
        path.to_string()
    } else {
        format!("file://{path}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_to_language() {
        assert_eq!(extension_to_language("rs"), "rust");
        assert_eq!(extension_to_language("py"), "python");
        assert_eq!(extension_to_language("ts"), "typescript");
        assert_eq!(extension_to_language("xyz"), "xyz");
    }

    #[test]
    fn test_path_to_uri() {
        assert_eq!(path_to_uri("/foo/bar.rs"), "file:///foo/bar.rs");
        assert_eq!(
            path_to_uri("file:///already"),
            "file:///already"
        );
    }

    #[test]
    fn test_manager_new_is_empty() {
        let mgr = LspManager::new();
        assert!(mgr.list_servers().is_empty());
    }

    #[test]
    fn test_manager_default() {
        let mgr = LspManager::default();
        assert!(mgr.list_servers().is_empty());
    }
}
