//! Event handling for the TUI application
//!
//! This module provides event processing and application state updates
//! for keyboard input, async operations, and system events.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use crate::sync::SyncSummary;

/// Events that can occur in the TUI application
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// User keyboard input
    KeyEvent(KeyEvent),
    /// Sync operation completed
    SyncCompleted(SyncSummary),
    /// Sync operation failed
    SyncFailed(String),
    /// Repository status updated
    StatusUpdate(String),
    /// Configuration reloaded
    ConfigReloaded,
    /// Application should exit
    Exit,
    /// Periodic tick for updates
    Tick,
}

/// Event handler for processing TUI events
pub struct EventHandler {
    /// Receiver for application events
    receiver: mpsc::UnboundedReceiver<AppEvent>,
    /// Sender for application events (for cloning)
    sender: mpsc::UnboundedSender<AppEvent>,
    /// Last tick time for periodic updates
    last_tick: Instant,
    /// Tick interval
    tick_interval: Duration,
}

impl EventHandler {
    /// Create a new event handler
    pub fn new(tick_interval: Duration) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        Self {
            receiver,
            sender,
            last_tick: Instant::now(),
            tick_interval,
        }
    }

    /// Get a sender handle for sending events
    pub fn sender(&self) -> mpsc::UnboundedSender<AppEvent> {
        self.sender.clone()
    }

    /// Get the next event, handling ticks automatically
    pub async fn next_event(&mut self) -> Result<AppEvent> {
        loop {
            // Check if we need to send a tick
            if self.last_tick.elapsed() >= self.tick_interval {
                self.last_tick = Instant::now();
                let _ = self.sender.send(AppEvent::Tick);
            }

            // Try to receive an event with a timeout
            match tokio::time::timeout(Duration::from_millis(50), self.receiver.recv()).await {
                Ok(Some(event)) => {
                    return Ok(event);
                }
                Ok(None) => {
                    // Channel closed
                    return Ok(AppEvent::Exit);
                }
                Err(_) => {
                    // Timeout - check for tick again
                    if self.last_tick.elapsed() >= self.tick_interval {
                        self.last_tick = Instant::now();
                        return Ok(AppEvent::Tick);
                    }
                    // Continue the loop to try again
                }
            }
        }
    }
}

/// Helper functions for key event processing
pub mod key_handler {
    use super::*;

    /// Check if a key event matches a specific key combination
    pub fn matches_key(event: &KeyEvent, code: KeyCode, modifiers: KeyModifiers) -> bool {
        event.code == code && event.modifiers == modifiers
    }

    /// Check if a key event is a simple key press (no modifiers)
    pub fn matches_simple_key(event: &KeyEvent, code: KeyCode) -> bool {
        matches_key(event, code, KeyModifiers::NONE)
    }

    /// Check if a key event is Ctrl+key combination
    pub fn matches_ctrl_key(event: &KeyEvent, code: KeyCode) -> bool {
        matches_key(event, code, KeyModifiers::CONTROL)
    }

    /// Convert key event to navigation action
    pub fn key_to_navigation(event: &KeyEvent) -> Option<NavigationAction> {
        match event.code {
            KeyCode::Up | KeyCode::Char('k') if event.modifiers.is_empty() => {
                Some(NavigationAction::Up)
            }
            KeyCode::Down | KeyCode::Char('j') if event.modifiers.is_empty() => {
                Some(NavigationAction::Down)
            }
            KeyCode::Left | KeyCode::Char('h') if event.modifiers.is_empty() => {
                Some(NavigationAction::Left)
            }
            KeyCode::Right | KeyCode::Char('l') if event.modifiers.is_empty() => {
                Some(NavigationAction::Right)
            }
            KeyCode::PageUp => Some(NavigationAction::PageUp),
            KeyCode::PageDown => Some(NavigationAction::PageDown),
            KeyCode::Home => Some(NavigationAction::Home),
            KeyCode::End => Some(NavigationAction::End),
            KeyCode::Enter => Some(NavigationAction::Select),
            KeyCode::Esc => Some(NavigationAction::Back),
            _ => None,
        }
    }

    /// Convert key event to application action
    pub fn key_to_app_action(event: &KeyEvent) -> Option<AppAction> {
        match (event.code, event.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::NONE) => Some(AppAction::Quit),
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(AppAction::Quit),
            (KeyCode::Char('r'), KeyModifiers::NONE) => Some(AppAction::Refresh),
            (KeyCode::Char('s'), KeyModifiers::NONE) => Some(AppAction::StartSync),
            (KeyCode::Char('d'), KeyModifiers::NONE) => Some(AppAction::StartDaemon),
            (KeyCode::Char('?'), KeyModifiers::NONE) => Some(AppAction::ShowHelp),
            (KeyCode::F(1), KeyModifiers::NONE) => Some(AppAction::ShowHelp),
            (KeyCode::Tab, KeyModifiers::NONE) => Some(AppAction::NextTab),
            (KeyCode::BackTab, KeyModifiers::NONE) => Some(AppAction::PreviousTab),
            (KeyCode::Char('\t'), KeyModifiers::NONE) => Some(AppAction::NextTab),
            _ => None,
        }
    }
}

/// Navigation actions within the TUI
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationAction {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
    Select,
    Back,
}

/// High-level application actions
#[derive(Debug, Clone, PartialEq)]
pub enum AppAction {
    Quit,
    Refresh,
    StartSync,
    StartDaemon,
    ShowHelp,
    NextTab,
    PreviousTab,
    ShowConfig,
    ShowLogs,
}

/// Event dispatcher for handling async operations
pub struct AsyncEventDispatcher {
    sender: mpsc::UnboundedSender<AppEvent>,
}

impl AsyncEventDispatcher {
    /// Create a new async event dispatcher
    pub fn new(sender: mpsc::UnboundedSender<AppEvent>) -> Self {
        Self { sender }
    }

    /// Send a sync completed event
    pub fn sync_completed(&self, summary: SyncSummary) {
        let _ = self.sender.send(AppEvent::SyncCompleted(summary));
    }

    /// Send a sync failed event
    pub fn sync_failed(&self, error: String) {
        let _ = self.sender.send(AppEvent::SyncFailed(error));
    }

    /// Send a status update event
    pub fn status_update(&self, message: String) {
        let _ = self.sender.send(AppEvent::StatusUpdate(message));
    }

    /// Send a configuration reloaded event
    pub fn config_reloaded(&self) {
        let _ = self.sender.send(AppEvent::ConfigReloaded);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_key_matching() {
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(key_handler::matches_simple_key(&event, KeyCode::Char('q')));
        assert!(!key_handler::matches_simple_key(&event, KeyCode::Char('r')));

        let ctrl_event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(key_handler::matches_ctrl_key(&ctrl_event, KeyCode::Char('c')));
        assert!(!key_handler::matches_simple_key(&ctrl_event, KeyCode::Char('c')));
    }

    #[test]
    fn test_navigation_actions() {
        let up_event = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(
            key_handler::key_to_navigation(&up_event),
            Some(NavigationAction::Up)
        );

        let k_event = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        assert_eq!(
            key_handler::key_to_navigation(&k_event),
            Some(NavigationAction::Up)
        );

        let invalid_event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(key_handler::key_to_navigation(&invalid_event), None);
    }

    #[test]
    fn test_app_actions() {
        let quit_event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(
            key_handler::key_to_app_action(&quit_event),
            Some(AppAction::Quit)
        );

        let ctrl_c_event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(
            key_handler::key_to_app_action(&ctrl_c_event),
            Some(AppAction::Quit)
        );

        let tab_event = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(
            key_handler::key_to_app_action(&tab_event),
            Some(AppAction::NextTab)
        );
    }

    #[tokio::test]
    async fn test_event_handler() {
        let mut handler = EventHandler::new(Duration::from_millis(100));
        let sender = handler.sender();

        // Send a test event
        sender.send(AppEvent::Exit).unwrap();

        // Receive the event
        let event = handler.next_event().await.unwrap();
        assert!(matches!(event, AppEvent::Exit));
    }
}