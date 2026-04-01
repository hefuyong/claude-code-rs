//! Bridge client for Claude Code RS.
//!
//! Provides a WebSocket-based bridge that connects the CLI to claude.ai,
//! handling authentication, session management, and bidirectional messaging.

pub mod client;
pub mod config;
pub mod device_trust;
pub mod jwt;
pub mod messaging;
pub mod session;

pub use client::BridgeClient;
pub use config::BridgeConfig;
pub use session::BridgeSession;
