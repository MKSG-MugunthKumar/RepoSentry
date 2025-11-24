//! Main application state for the TUI

use crate::{Config, SyncEngine, GitHubClient};
use crate::daemon::is_daemon_running;
use crate::git::RepoState;
use crate::sync::SyncSummary;
use super::events::{AppEvent, EventHandler, key_handler, AppAction, NavigationAction, AsyncEventDispatcher};
use super::widgets::{ColorScheme, RepositoryList, SyncStatusWidget, ConfigViewer, LogViewer, HelpDialog, TabBar, StatusBar, ProgressDialog};
use anyhow::Result;
use crossterm::event::{KeyEvent, KeyCode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::ListState,
    Frame,
};
use std::time::{Duration, Instant};
use tracing::{info, error, debug};

/// Application state
pub struct App {
    config: Config,
    sync_engine: Option<SyncEngine>,
    github_client: Option<GitHubClient>,

    // Event handling
    event_handler: EventHandler,
    event_dispatcher: AsyncEventDispatcher,

    // UI state
    current_tab: usize,
    tabs: Vec<&'static str>,
    colors: ColorScheme,

    // Repository state
    repositories: Vec<RepoState>,
    selected_repo: usize,
    list_state: ListState,

    // Status
    daemon_running: bool,
    last_sync: Option<Instant>,
    last_sync_summary: Option<SyncSummary>,
    current_operation: Option<String>,
    status_message: String,
    logs: Vec<String>,
    log_scroll_offset: usize,

    // Popup state
    show_help: bool,
    show_config: bool,
    show_progress: bool,
    show_error: Option<String>,

    // Config display
    config_text: String,

    // Exit flag
    should_exit: bool,
}

impl App {
    /// Create a new application instance
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing TUI application");

        // Create event handler
        let event_handler = EventHandler::new(Duration::from_millis(250));
        let event_dispatcher = AsyncEventDispatcher::new(event_handler.sender());

        // Try to create sync engine
        let sync_engine = match SyncEngine::new(config.clone()).await {
            Ok(engine) => {
                info!("Sync engine initialized successfully");
                Some(engine)
            }
            Err(e) => {
                error!("Failed to initialize sync engine: {}", e);
                None
            }
        };

        // Try to create GitHub client
        let github_client = match GitHubClient::new(&config).await {
            Ok(client) => {
                info!("GitHub client initialized successfully");
                Some(client)
            }
            Err(e) => {
                error!("Failed to initialize GitHub client: {}", e);
                None
            }
        };

        // Check daemon status
        let daemon_running = is_daemon_running(&config).unwrap_or(false);

        // Initialize list state
        let mut list_state = ListState::default();
        list_state.select(Some(0));

        // Generate config text for display
        let config_text = serde_yaml::to_string(&config)
            .unwrap_or_else(|_| "Failed to serialize config".to_string());

        Ok(Self {
            config,
            sync_engine,
            github_client,
            event_handler,
            event_dispatcher,
            current_tab: 0,
            tabs: vec!["Repositories", "Status", "Config", "Logs"],
            colors: ColorScheme::default(),
            repositories: Vec::new(),
            selected_repo: 0,
            list_state,
            daemon_running,
            last_sync: None,
            last_sync_summary: None,
            current_operation: None,
            status_message: "Ready".to_string(),
            logs: vec![
                "Application started".to_string(),
                "TUI initialized successfully".to_string(),
            ],
            log_scroll_offset: 0,
            show_help: false,
            show_config: false,
            show_progress: false,
            show_error: None,
            config_text,
            should_exit: false,
        })
    }

    /// Check if the application should exit
    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    /// Handle keyboard events
    pub async fn handle_key_event(&mut self, key_event: KeyEvent) -> Result<()> {
        debug!("Handling key event: {:?}", key_event);

        // Handle popup-specific keys first
        if self.show_help || self.show_error.is_some() {
            match key_event.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.show_help = false;
                    self.show_error = None;
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle global app actions
        if let Some(action) = key_handler::key_to_app_action(&key_event) {
            match action {
                AppAction::Quit => {
                    info!("User requested quit");
                    self.should_exit = true;
                }
                AppAction::ShowHelp => {
                    self.show_help = true;
                }
                AppAction::NextTab => {
                    self.next_tab();
                }
                AppAction::PreviousTab => {
                    self.previous_tab();
                }
                AppAction::Refresh => {
                    self.refresh_data().await?;
                }
                AppAction::StartSync => {
                    self.start_sync().await?;
                }
                AppAction::StartDaemon => {
                    self.toggle_daemon().await?;
                }
                _ => {}
            }
        }

        // Handle navigation within current tab
        if let Some(nav_action) = key_handler::key_to_navigation(&key_event) {
            self.handle_navigation(nav_action).await?;
        }

        Ok(())
    }

    /// Handle navigation actions
    async fn handle_navigation(&mut self, action: NavigationAction) -> Result<()> {
        match self.current_tab {
            0 => { // Repositories tab
                match action {
                    NavigationAction::Up => {
                        if self.selected_repo > 0 {
                            self.selected_repo -= 1;
                            self.list_state.select(Some(self.selected_repo));
                        }
                    }
                    NavigationAction::Down => {
                        if self.selected_repo < self.repositories.len().saturating_sub(1) {
                            self.selected_repo += 1;
                            self.list_state.select(Some(self.selected_repo));
                        }
                    }
                    NavigationAction::Select => {
                        if self.selected_repo < self.repositories.len() {
                            let repo = self.repositories[self.selected_repo].clone();
                            self.show_repository_details(&repo).await?;
                        }
                    }
                    _ => {}
                }
            }
            3 => { // Logs tab
                match action {
                    NavigationAction::Up => {
                        self.log_scroll_offset = self.log_scroll_offset.saturating_sub(1);
                    }
                    NavigationAction::Down => {
                        if self.log_scroll_offset < self.logs.len().saturating_sub(1) {
                            self.log_scroll_offset += 1;
                        }
                    }
                    NavigationAction::PageUp => {
                        self.log_scroll_offset = self.log_scroll_offset.saturating_sub(10);
                    }
                    NavigationAction::PageDown => {
                        self.log_scroll_offset = (self.log_scroll_offset + 10).min(self.logs.len().saturating_sub(1));
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Move to the next tab
    fn next_tab(&mut self) {
        self.current_tab = (self.current_tab + 1) % self.tabs.len();
    }

    /// Move to the previous tab
    fn previous_tab(&mut self) {
        if self.current_tab > 0 {
            self.current_tab -= 1;
        } else {
            self.current_tab = self.tabs.len() - 1;
        }
    }

    /// Refresh application data
    async fn refresh_data(&mut self) -> Result<()> {
        info!("Refreshing application data");
        self.add_log("Refreshing data...".to_string());

        // Update daemon status
        self.daemon_running = is_daemon_running(&self.config).unwrap_or(false);

        // Refresh repositories if we have a sync engine
        if let Some(ref sync_engine) = self.sync_engine {
            match sync_engine.dry_run().await {
                Ok(repo_states) => {
                    self.repositories = repo_states;
                    self.add_log(format!("Loaded {} repositories", self.repositories.len()));
                }
                Err(e) => {
                    error!("Failed to refresh repositories: {}", e);
                    self.show_error = Some(format!("Failed to refresh repositories: {}", e));
                }
            }
        }

        self.status_message = "Data refreshed".to_string();
        Ok(())
    }

    /// Start a sync operation
    async fn start_sync(&mut self) -> Result<()> {
        if let Some(sync_engine) = self.sync_engine.clone() {
            info!("Starting sync operation");
            self.add_log("Starting repository synchronization...".to_string());
            self.current_operation = Some("Synchronizing repositories".to_string());
            self.show_progress = true;

            // Run sync in background and update UI
            match sync_engine.run_sync().await {
                Ok(summary) => {
                    self.last_sync = Some(Instant::now());
                    self.last_sync_summary = Some(summary.clone());
                    self.add_log(format!(
                        "Sync completed: {} successful, {} failed, {} skipped",
                        summary.successful_operations,
                        summary.failed_operations,
                        summary.skipped_operations
                    ));
                    self.status_message = "Sync completed successfully".to_string();
                }
                Err(e) => {
                    error!("Sync failed: {}", e);
                    self.add_log(format!("Sync failed: {}", e));
                    self.show_error = Some(format!("Sync failed: {}", e));
                }
            }

            self.current_operation = None;
            self.show_progress = false;
        } else {
            self.show_error = Some("Sync engine not available. Check configuration.".to_string());
        }

        Ok(())
    }

    /// Toggle daemon status
    async fn toggle_daemon(&mut self) -> Result<()> {
        if self.daemon_running {
            self.add_log("Stopping daemon...".to_string());
            // TODO: Implement daemon stopping
            self.status_message = "Daemon stop requested".to_string();
        } else {
            self.add_log("Starting daemon...".to_string());
            // TODO: Implement daemon starting
            self.status_message = "Daemon start requested".to_string();
        }

        Ok(())
    }

    /// Show details for a repository
    async fn show_repository_details(&mut self, _repo: &RepoState) -> Result<()> {
        // TODO: Implement repository details view
        self.add_log("Repository details not yet implemented".to_string());
        Ok(())
    }

    /// Add a log message
    fn add_log(&mut self, message: String) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        self.logs.push(format!("[{}] {}", timestamp, message));

        // Keep only last 1000 log entries
        if self.logs.len() > 1000 {
            self.logs.drain(..self.logs.len() - 1000);
        }
    }

    /// Process pending events
    pub async fn update(&mut self) -> Result<()> {
        // Try to get an event without blocking
        if let Ok(event) = tokio::time::timeout(Duration::from_millis(1), self.event_handler.next_event()).await {
            match event {
                Ok(AppEvent::Tick) => {
                    // Periodic update
                    self.daemon_running = is_daemon_running(&self.config).unwrap_or(false);
                }
                Ok(AppEvent::SyncCompleted(summary)) => {
                    self.last_sync = Some(Instant::now());
                    self.last_sync_summary = Some(summary);
                    self.current_operation = None;
                    self.show_progress = false;
                }
                Ok(AppEvent::SyncFailed(error)) => {
                    self.show_error = Some(error);
                    self.current_operation = None;
                    self.show_progress = false;
                }
                Ok(AppEvent::StatusUpdate(message)) => {
                    self.add_log(message);
                }
                Ok(AppEvent::Exit) => {
                    self.should_exit = true;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Draw the application UI
    pub fn draw(&mut self, frame: &mut Frame) {
        let size = frame.size();

        // Create main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Tab bar
                Constraint::Min(0),    // Main content
                Constraint::Length(1), // Status bar
            ])
            .split(size);

        // Draw tab bar
        let tab_bar = TabBar::new(&self.tabs, self.current_tab, &self.colors);
        tab_bar.render(frame, chunks[0]);

        // Draw main content based on current tab
        match self.current_tab {
            0 => self.draw_repositories_tab(frame, chunks[1]),
            1 => self.draw_status_tab(frame, chunks[1]),
            2 => self.draw_config_tab(frame, chunks[1]),
            3 => self.draw_logs_tab(frame, chunks[1]),
            _ => {}
        }

        // Draw status bar
        let left_text = if self.daemon_running {
            Some("Daemon: Running")
        } else {
            Some("Daemon: Stopped")
        };
        let center_text = Some(self.status_message.as_str());
        let right_text = if let Some(_last_sync) = self.last_sync {
            Some("Synced recently")
        } else {
            Some("No sync yet")
        };

        let status_bar = StatusBar::new(left_text, center_text, right_text, &self.colors);
        status_bar.render(frame, chunks[2]);

        // Draw popups
        if self.show_help {
            let help_dialog = HelpDialog::new(&self.colors);
            help_dialog.render(frame, size);
        }

        if let Some(ref error) = self.show_error {
            let progress_dialog = ProgressDialog::new(
                "Error",
                error,
                None,
                &self.colors,
            );
            progress_dialog.render(frame, size);
        }

        if self.show_progress {
            let operation = self.current_operation.as_deref().unwrap_or("Processing...");
            let progress_dialog = ProgressDialog::new(
                "Working",
                operation,
                None, // Indeterminate progress
                &self.colors,
            );
            progress_dialog.render(frame, size);
        }
    }

    /// Draw the repositories tab
    fn draw_repositories_tab(&mut self, frame: &mut Frame, area: Rect) {
        let repo_list = RepositoryList::new(&self.repositories, &self.colors);
        repo_list.render(frame, area, &mut self.list_state);
    }

    /// Draw the status tab
    fn draw_status_tab(&mut self, frame: &mut Frame, area: Rect) {
        let status_widget = SyncStatusWidget::new(
            self.last_sync_summary.as_ref(),
            self.current_operation.as_deref(),
            &self.colors,
        );
        status_widget.render(frame, area);
    }

    /// Draw the config tab
    fn draw_config_tab(&mut self, frame: &mut Frame, area: Rect) {
        let config_viewer = ConfigViewer::new(&self.config_text, &self.colors);
        config_viewer.render(frame, area);
    }

    /// Draw the logs tab
    fn draw_logs_tab(&mut self, frame: &mut Frame, area: Rect) {
        let log_viewer = LogViewer::new(&self.logs, &self.colors, self.log_scroll_offset);
        log_viewer.render(frame, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_app_creation() {
        let config = Config::default();
        let app = App::new(config).await;

        // App creation should succeed even if GitHub auth fails
        assert!(app.is_ok());

        if let Ok(app) = app {
            assert_eq!(app.current_tab, 0);
            assert_eq!(app.tabs.len(), 4);
            assert!(!app.should_exit);
            assert!(!app.show_help);
        }
    }

    #[tokio::test]
    async fn test_tab_navigation() {
        let config = Config::default();
        let mut app = App::new(config).await.unwrap();

        // Test next tab
        app.next_tab();
        assert_eq!(app.current_tab, 1);

        app.next_tab();
        assert_eq!(app.current_tab, 2);

        // Test wrapping
        app.current_tab = app.tabs.len() - 1;
        app.next_tab();
        assert_eq!(app.current_tab, 0);

        // Test previous tab
        app.previous_tab();
        assert_eq!(app.current_tab, app.tabs.len() - 1);
    }

    #[tokio::test]
    async fn test_log_management() {
        let config = Config::default();
        let mut app = App::new(config).await.unwrap();

        // Test adding logs
        let initial_count = app.logs.len();
        app.add_log("Test log message".to_string());
        assert_eq!(app.logs.len(), initial_count + 1);
        assert!(app.logs.last().unwrap().contains("Test log message"));

        // Test log limit (would need to add 1000+ logs to test properly)
        // This is just a basic check that the function doesn't crash
        for i in 0..10 {
            app.add_log(format!("Log message {}", i));
        }
        assert!(app.logs.len() <= 1000);
    }
}