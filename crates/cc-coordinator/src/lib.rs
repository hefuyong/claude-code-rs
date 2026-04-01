//! Multi-agent coordinator for Claude Code RS.
//!
//! Provides infrastructure for spawning, managing, and orchestrating
//! multiple concurrent worker agents that collaborate on tasks.

pub mod coordinator;
pub mod routing;
pub mod worker;

pub use coordinator::Coordinator;
pub use worker::{Worker, WorkerConfig};
