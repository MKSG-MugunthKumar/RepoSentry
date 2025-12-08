//! Terminal User Interface for RepoSentry
//!
//! This module provides an interactive TUI for managing repository synchronization,
//! viewing status, and configuring settings using ratatui and crossterm.

pub mod app;
pub mod events;
pub mod widgets;

use crate::Config;
use anyhow::Result;
use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;

/// Launch the TUI application
pub async fn run_tui(config: Config) -> Result<()> {
    // Create app state BEFORE entering raw mode
    // This allows any initialization logs to display normally
    let mut app = App::new(config).await?;

    // Setup terminal (raw mode)
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Main event loop
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Main application event loop
async fn run_app<B>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()>
where
    B: ratatui::backend::Backend,
{
    loop {
        // Draw the UI
        terminal.draw(|f| app.draw(f))?;

        // Handle events
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        break;
                    }
                    _ => {
                        app.handle_key_event(key).await?;
                    }
                }
            }
        }

        // Periodic updates
        app.update().await?;
    }

    Ok(())
}