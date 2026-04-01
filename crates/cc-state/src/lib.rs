//! Application state store for Claude Code RS.
//!
//! Provides a thread-safe, observable state container that the TUI
//! and other subsystems can read and update. Subscribers are notified
//! via a [`tokio::sync::watch`] channel whenever the state changes.

use std::sync::Arc;
use tokio::sync::{watch, RwLock};

/// The global application state shared across the TUI, query loop, and
/// other subsystems.
pub struct AppState {
    /// The model identifier currently in use.
    pub model: String,
    /// Whether verbose logging is enabled.
    pub verbose: bool,
    /// The active permission mode.
    pub permission_mode: cc_permissions::PermissionMode,
    /// The working directory for tool execution and path resolution.
    pub working_directory: std::path::PathBuf,
    /// The current session id, if a session is active.
    pub session_id: Option<cc_types::SessionId>,
    /// Cumulative API cost in USD for this session.
    pub total_cost_usd: f64,
    /// Total tokens (input + output) consumed so far.
    pub total_tokens: u64,
    /// Number of completed query turns.
    pub turn_count: u32,
    /// An optional status line shown in the UI (e.g. "Thinking...", "Running bash...").
    pub status_text: Option<String>,
}

impl AppState {
    /// Create a new state with sensible defaults.
    pub fn new(model: String, working_directory: std::path::PathBuf) -> Self {
        Self {
            model,
            verbose: false,
            permission_mode: cc_permissions::PermissionMode::Default,
            working_directory,
            session_id: None,
            total_cost_usd: 0.0,
            total_tokens: 0,
            turn_count: 0,
            status_text: None,
        }
    }
}

/// A thread-safe, observable wrapper around [`AppState`].
///
/// Readers acquire an async `RwLock` read guard. Writers pass a closure
/// to [`update`], which takes an exclusive lock, applies the mutation,
/// and bumps a version counter that wakes all subscribers.
pub struct AppStateStore {
    state: Arc<RwLock<AppState>>,
    version_tx: watch::Sender<u64>,
    version_rx: watch::Receiver<u64>,
}

impl AppStateStore {
    /// Create a new store with the given initial state.
    pub fn new(initial: AppState) -> Self {
        let (version_tx, version_rx) = watch::channel(0u64);
        Self {
            state: Arc::new(RwLock::new(initial)),
            version_tx,
            version_rx,
        }
    }

    /// Acquire a read guard on the state.
    pub async fn get(&self) -> tokio::sync::RwLockReadGuard<'_, AppState> {
        self.state.read().await
    }

    /// Apply a mutation to the state and notify subscribers.
    pub async fn update(&self, f: impl FnOnce(&mut AppState)) {
        let mut state = self.state.write().await;
        f(&mut state);
        // Bump the version; ignore send errors (no receivers is fine).
        let new_version = *self.version_rx.borrow() + 1;
        let _ = self.version_tx.send(new_version);
        tracing::trace!(version = new_version, "state updated");
    }

    /// Obtain a watch receiver that yields a new version number each
    /// time the state changes. Useful for driving UI redraws.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.version_rx.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn basic_read_write() {
        let store = AppStateStore::new(AppState::new(
            "sonnet".into(),
            std::path::PathBuf::from("."),
        ));
        {
            let state = store.get().await;
            assert_eq!(state.model, "sonnet");
            assert_eq!(state.turn_count, 0);
        }
        store.update(|s| s.turn_count = 5).await;
        {
            let state = store.get().await;
            assert_eq!(state.turn_count, 5);
        }
    }

    #[tokio::test]
    async fn subscribe_receives_updates() {
        let store = AppStateStore::new(AppState::new(
            "haiku".into(),
            std::path::PathBuf::from("/tmp"),
        ));
        let mut rx = store.subscribe();
        assert_eq!(*rx.borrow(), 0);

        store.update(|s| s.total_cost_usd = 0.01).await;
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), 1);

        store.update(|s| s.total_tokens = 100).await;
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), 2);
    }
}
