//! Event loop for the TUI.
//!
//! Merges crossterm terminal events (keyboard, mouse, resize), query
//! events from the agentic loop, and periodic tick events into a
//! single async stream consumed by the main TUI loop.

use cc_query::QueryEvent;
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyEvent, MouseEvent,
    MouseEventKind,
};
use futures::StreamExt;
use std::time::Duration;
use tokio::sync::mpsc;

/// Events consumed by the main TUI loop.
pub enum AppEvent {
    /// A keyboard event from the terminal.
    Key(KeyEvent),
    /// A mouse event (scroll, click).
    Mouse(MouseEvent),
    /// The terminal was resized.
    Resize(u16, u16),
    /// A query event from the agentic loop.
    QueryEvent(QueryEvent),
    /// Periodic tick (used for spinner animation, status updates).
    Tick,
}

/// Drives the event loop, merging terminal input, query events, and ticks.
pub struct EventLoop {
    /// Receives terminal + tick events.
    event_rx: mpsc::UnboundedReceiver<AppEvent>,
    /// Sender that the query loop can clone to push events.
    query_tx: mpsc::UnboundedSender<AppEvent>,
}

impl EventLoop {
    /// Create a new event loop. Spawns background tasks that read
    /// crossterm events (keyboard + mouse + resize) and generate ticks.
    ///
    /// The tick interval is 100 ms so the spinner animates smoothly.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let query_tx = tx.clone();

        // Enable mouse capture so we can handle scroll events.
        let _ = crossterm::execute!(std::io::stdout(), EnableMouseCapture);

        // Spawn the terminal event reader.
        let terminal_tx = tx.clone();
        tokio::spawn(async move {
            let mut reader = EventStream::new();
            loop {
                match reader.next().await {
                    Some(Ok(Event::Key(key))) => {
                        if terminal_tx.send(AppEvent::Key(key)).is_err() {
                            break;
                        }
                    }
                    Some(Ok(Event::Mouse(mouse))) => {
                        // Only forward scroll events -- ignore raw moves.
                        match mouse.kind {
                            MouseEventKind::ScrollUp
                            | MouseEventKind::ScrollDown
                            | MouseEventKind::ScrollLeft
                            | MouseEventKind::ScrollRight => {
                                if terminal_tx.send(AppEvent::Mouse(mouse)).is_err() {
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(Ok(Event::Resize(w, h))) => {
                        if terminal_tx.send(AppEvent::Resize(w, h)).is_err() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, "crossterm event error");
                        break;
                    }
                    None => break,
                }
            }
        });

        // Spawn the tick generator (100 ms for smooth spinner animation).
        let tick_tx = tx;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(100));
            loop {
                interval.tick().await;
                if tick_tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });

        Self {
            event_rx: rx,
            query_tx,
        }
    }

    /// Wait for the next event. Returns `None` when all senders are dropped.
    pub async fn next(&mut self) -> Option<AppEvent> {
        self.event_rx.recv().await
    }

    /// Get a sender that can be used to push [`QueryEvent`]s from
    /// another task (e.g. the agentic query loop).
    pub fn query_sender(&self) -> mpsc::UnboundedSender<AppEvent> {
        self.query_tx.clone()
    }
}

impl Default for EventLoop {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for EventLoop {
    fn drop(&mut self) {
        // Restore the terminal mouse state.
        let _ = crossterm::execute!(std::io::stdout(), DisableMouseCapture);
    }
}
