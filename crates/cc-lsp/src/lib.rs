//! LSP (Language Server Protocol) integration for Claude Code RS.
//!
//! Provides the ability to launch and communicate with language servers
//! (e.g. rust-analyzer, typescript-language-server, pyright) over the
//! standard LSP stdio transport. Diagnostic information from these
//! servers can be aggregated and surfaced to the AI assistant so that
//! it is aware of compile errors and warnings in the workspace.

pub mod config;
pub mod diagnostics;
pub mod manager;
pub mod protocol;
pub mod server;

// Re-exports for convenience.
pub use config::LspServerConfig;
pub use diagnostics::{DiagnosticRegistry, DiagnosticSummary};
pub use manager::LspManager;
pub use protocol::{
    CompletionItem, Diagnostic, DiagnosticSeverity, Location, Position, Range,
};
pub use server::{LspServer, ServerStatus};
