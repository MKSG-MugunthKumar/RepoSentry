//! Reusable widgets for the TUI application
//!
//! This module provides custom widgets and UI components that can be used
//! across different parts of the TUI interface.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame,
};

use crate::{git::RepoState, sync::SyncSummary};

/// Color scheme for the TUI
pub struct ColorScheme {
    pub primary: Color,
    pub secondary: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    pub text: Color,
    pub background: Color,
    pub border: Color,
}

impl Default for ColorScheme {
    fn default() -> Self {
        Self {
            primary: Color::Blue,
            secondary: Color::Cyan,
            success: Color::Green,
            warning: Color::Yellow,
            error: Color::Red,
            info: Color::Magenta,
            text: Color::White,
            background: Color::Black,
            border: Color::Gray,
        }
    }
}

/// Repository list widget with status indicators
pub struct RepositoryList<'a> {
    repositories: &'a [RepoState],
    colors: &'a ColorScheme,
}

impl<'a> RepositoryList<'a> {
    pub fn new(repositories: &'a [RepoState], colors: &'a ColorScheme) -> Self {
        Self {
            repositories,
            colors,
        }
    }

    /// Render the repository list widget
    pub fn render(&self, frame: &mut Frame, area: Rect, state: &mut ListState) {
        let items: Vec<ListItem> = self
            .repositories
            .iter()
            .map(|repo| {
                let (status_icon, status_color) = if !repo.exists {
                    ("📥", self.colors.info) // Clone needed
                } else if repo.has_uncommitted_changes {
                    ("⚠️", self.colors.warning) // Uncommitted changes
                } else if repo.has_conflicts {
                    ("⚡", self.colors.error) // Conflicts
                } else if repo.is_behind_remote {
                    ("↓", self.colors.info) // Behind remote
                } else if repo.is_ahead_of_remote {
                    ("↑", self.colors.secondary) // Ahead of remote
                } else {
                    ("✓", self.colors.success) // Up to date
                };

                let path_str = repo
                    .path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown");

                let branch_info = if let Some(ref branch) = repo.current_branch {
                    format!(" ({})", branch)
                } else {
                    " (no branch)".to_string()
                };

                let content = Line::from(vec![
                    Span::styled(
                        format!("{} ", status_icon),
                        Style::default()
                            .fg(status_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(path_str, Style::default().fg(self.colors.text)),
                    Span::styled(branch_info, Style::default().fg(self.colors.secondary)),
                ]);

                ListItem::new(content)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title("Repositories")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.colors.border)),
            )
            .highlight_style(
                Style::default()
                    .bg(self.colors.primary)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, area, state);
    }
}

/// Sync status widget showing progress and results
pub struct SyncStatusWidget<'a> {
    summary: Option<&'a SyncSummary>,
    current_operation: Option<&'a str>,
    colors: &'a ColorScheme,
}

impl<'a> SyncStatusWidget<'a> {
    pub fn new(
        summary: Option<&'a SyncSummary>,
        current_operation: Option<&'a str>,
        colors: &'a ColorScheme,
    ) -> Self {
        Self {
            summary,
            current_operation,
            colors,
        }
    }

    /// Render the sync status widget
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title("Sync Status")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.colors.border));

        if let Some(summary) = self.summary {
            let success_rate = if summary.total_repositories > 0 {
                summary.successful_operations as f64 / summary.total_repositories as f64
            } else {
                0.0
            };

            let gauge_color = if success_rate >= 0.9 {
                self.colors.success
            } else if success_rate >= 0.7 {
                self.colors.warning
            } else {
                self.colors.error
            };

            let gauge = Gauge::default()
                .block(block)
                .gauge_style(Style::default().fg(gauge_color))
                .percent((success_rate * 100.0) as u16)
                .label(format!(
                    "{}/{} successful ({:.1}%)",
                    summary.successful_operations,
                    summary.total_repositories,
                    success_rate * 100.0
                ));

            frame.render_widget(gauge, area);
        } else if let Some(operation) = self.current_operation {
            let paragraph = Paragraph::new(Text::from(format!("Running: {}", operation)))
                .block(block)
                .style(Style::default().fg(self.colors.info))
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, area);
        } else {
            let paragraph = Paragraph::new(Text::from("Ready"))
                .block(block)
                .style(Style::default().fg(self.colors.text))
                .alignment(Alignment::Center);

            frame.render_widget(paragraph, area);
        }
    }
}

/// Configuration viewer widget
pub struct ConfigViewer<'a> {
    config_text: &'a str,
    colors: &'a ColorScheme,
}

impl<'a> ConfigViewer<'a> {
    pub fn new(config_text: &'a str, colors: &'a ColorScheme) -> Self {
        Self {
            config_text,
            colors,
        }
    }

    /// Render the configuration viewer widget
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let paragraph = Paragraph::new(Text::from(self.config_text))
            .block(
                Block::default()
                    .title("Configuration")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.colors.border)),
            )
            .style(Style::default().fg(self.colors.text))
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }
}

/// Log viewer widget with scrolling capability
pub struct LogViewer<'a> {
    logs: &'a [String],
    colors: &'a ColorScheme,
    scroll_offset: usize,
}

impl<'a> LogViewer<'a> {
    pub fn new(logs: &'a [String], colors: &'a ColorScheme, scroll_offset: usize) -> Self {
        Self {
            logs,
            colors,
            scroll_offset,
        }
    }

    /// Render the log viewer widget
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
        let start_idx = self.scroll_offset.min(self.logs.len().saturating_sub(1));
        let end_idx = (start_idx + visible_height).min(self.logs.len());

        let visible_logs = if start_idx < self.logs.len() {
            &self.logs[start_idx..end_idx]
        } else {
            &[]
        };

        let items: Vec<ListItem> = visible_logs
            .iter()
            .map(|log| {
                let color = if log.contains("ERROR") {
                    self.colors.error
                } else if log.contains("WARN") {
                    self.colors.warning
                } else if log.contains("INFO") {
                    self.colors.info
                } else {
                    self.colors.text
                };

                ListItem::new(Line::from(Span::styled(
                    log.as_str(),
                    Style::default().fg(color),
                )))
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .title(format!(
                    "Logs ({}/{})",
                    self.scroll_offset + 1,
                    self.logs.len()
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.colors.border)),
        );

        frame.render_widget(list, area);
    }
}

/// Help dialog widget
pub struct HelpDialog<'a> {
    colors: &'a ColorScheme,
}

impl<'a> HelpDialog<'a> {
    pub fn new(colors: &'a ColorScheme) -> Self {
        Self { colors }
    }

    /// Render the help dialog
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let popup_area = Self::centered_rect(60, 70, area);

        // Clear the background
        frame.render_widget(Clear, popup_area);

        let help_text = Text::from(vec![
            Line::from(vec![Span::styled(
                "Keyboard Shortcuts",
                Style::default()
                    .fg(self.colors.primary)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
            Line::from("Navigation:"),
            Line::from("  ↑/k        Move up"),
            Line::from("  ↓/j        Move down"),
            Line::from("  ←/h        Move left"),
            Line::from("  →/l        Move right"),
            Line::from("  Tab        Next tab"),
            Line::from("  Shift+Tab  Previous tab"),
            Line::from(""),
            Line::from("Actions:"),
            Line::from("  r          Refresh"),
            Line::from("  s          Start sync"),
            Line::from("  d          Start daemon"),
            Line::from("  Enter      Select"),
            Line::from("  Esc        Back/Cancel"),
            Line::from(""),
            Line::from("General:"),
            Line::from("  ?/F1       Show this help"),
            Line::from("  q/Ctrl+C   Quit"),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Press Esc to close",
                Style::default().fg(self.colors.secondary),
            )]),
        ]);

        let paragraph = Paragraph::new(help_text)
            .block(
                Block::default()
                    .title("Help")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.colors.border)),
            )
            .style(Style::default().fg(self.colors.text))
            .alignment(Alignment::Left);

        frame.render_widget(paragraph, popup_area);
    }

    /// Helper to create a centered rectangle
    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }
}

/// Tab bar widget for navigation
pub struct TabBar<'a> {
    tabs: &'a [&'a str],
    selected: usize,
    colors: &'a ColorScheme,
}

impl<'a> TabBar<'a> {
    pub fn new(tabs: &'a [&'a str], selected: usize, colors: &'a ColorScheme) -> Self {
        Self {
            tabs,
            selected,
            colors,
        }
    }

    /// Render the tab bar widget
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let titles: Vec<Line> = self.tabs.iter().map(|&t| Line::from(t)).collect();

        let tabs = Tabs::new(titles)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(self.colors.border)),
            )
            .style(Style::default().fg(self.colors.text))
            .highlight_style(
                Style::default()
                    .fg(self.colors.primary)
                    .add_modifier(Modifier::BOLD),
            )
            .select(self.selected);

        frame.render_widget(tabs, area);
    }
}

/// Status bar widget showing current status
pub struct StatusBar<'a> {
    left_text: Option<&'a str>,
    center_text: Option<&'a str>,
    right_text: Option<&'a str>,
    colors: &'a ColorScheme,
}

impl<'a> StatusBar<'a> {
    pub fn new(
        left_text: Option<&'a str>,
        center_text: Option<&'a str>,
        right_text: Option<&'a str>,
        colors: &'a ColorScheme,
    ) -> Self {
        Self {
            left_text,
            center_text,
            right_text,
            colors,
        }
    }

    /// Render the status bar widget
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(33),
                Constraint::Percentage(34),
                Constraint::Percentage(33),
            ])
            .split(area);

        // Left text
        if let Some(text) = self.left_text {
            let paragraph = Paragraph::new(Text::from(text))
                .style(Style::default().fg(self.colors.text))
                .alignment(Alignment::Left);
            frame.render_widget(paragraph, chunks[0]);
        }

        // Center text
        if let Some(text) = self.center_text {
            let paragraph = Paragraph::new(Text::from(text))
                .style(Style::default().fg(self.colors.primary))
                .alignment(Alignment::Center);
            frame.render_widget(paragraph, chunks[1]);
        }

        // Right text
        if let Some(text) = self.right_text {
            let paragraph = Paragraph::new(Text::from(text))
                .style(Style::default().fg(self.colors.secondary))
                .alignment(Alignment::Right);
            frame.render_widget(paragraph, chunks[2]);
        }
    }
}

/// Progress dialog for long-running operations
pub struct ProgressDialog<'a> {
    title: &'a str,
    message: &'a str,
    progress: Option<f64>, // 0.0 to 1.0, None for indeterminate
    colors: &'a ColorScheme,
}

impl<'a> ProgressDialog<'a> {
    pub fn new(
        title: &'a str,
        message: &'a str,
        progress: Option<f64>,
        colors: &'a ColorScheme,
    ) -> Self {
        Self {
            title,
            message,
            progress,
            colors,
        }
    }

    /// Render the progress dialog
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let popup_area = HelpDialog::centered_rect(50, 20, area);

        // Clear the background
        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(self.title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.colors.border));

        if let Some(progress) = self.progress {
            let gauge = Gauge::default()
                .block(block)
                .gauge_style(Style::default().fg(self.colors.primary))
                .percent((progress * 100.0) as u16)
                .label(self.message);

            frame.render_widget(gauge, popup_area);
        } else {
            let paragraph = Paragraph::new(Text::from(self.message))
                .block(block)
                .style(Style::default().fg(self.colors.text))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });

            frame.render_widget(paragraph, popup_area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_color_scheme_default() {
        let colors = ColorScheme::default();
        assert_eq!(colors.primary, Color::Blue);
        assert_eq!(colors.success, Color::Green);
        assert_eq!(colors.error, Color::Red);
    }

    #[test]
    fn test_help_dialog_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = HelpDialog::centered_rect(60, 70, area);

        // Should be roughly centered
        assert!(centered.x > 0 && centered.x < area.width);
        assert!(centered.y > 0 && centered.y < area.height);
        assert!(centered.width > 0 && centered.width < area.width);
        assert!(centered.height > 0 && centered.height < area.height);
    }

    #[test]
    fn test_repository_list_creation() {
        let colors = ColorScheme::default();
        let repos = vec![RepoState {
            path: PathBuf::from("/test/repo"),
            exists: true,
            has_uncommitted_changes: false,
            has_untracked_files: false,
            is_ahead_of_remote: false,
            is_behind_remote: false,
            has_conflicts: false,
            remote_url: Some("https://github.com/test/repo".to_string()),
            current_branch: Some("main".to_string()),
        }];

        let list = RepositoryList::new(&repos, &colors);
        assert_eq!(list.repositories.len(), 1);
    }
}
