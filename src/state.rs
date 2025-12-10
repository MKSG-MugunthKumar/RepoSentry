//! State Management - SQLite-based persistence for repository state and events
//!
//! This module provides persistent storage for:
//! - Repository sync state (current branch, last sync time, status)
//! - Sync events (branch switches, skipped repos, errors)
//!
//! The database is stored in XDG_DATA_HOME/reposentry/state.db

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::PathBuf;
use tracing::{debug, info};

/// Event types that can occur during sync operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    /// Repository was cloned for the first time
    Cloned,
    /// Repository was successfully pulled
    Pulled,
    /// Branch was switched to track more recent activity
    BranchSwitch,
    /// Repository was skipped due to local uncommitted changes
    SkippedLocalChanges,
    /// Repository was skipped due to unresolved conflicts
    SkippedConflicts,
    /// Repository was skipped because it's ahead of remote
    SkippedAheadOfRemote,
    /// Sync operation failed with an error
    SyncError,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::Cloned => "cloned",
            EventType::Pulled => "pulled",
            EventType::BranchSwitch => "branch_switch",
            EventType::SkippedLocalChanges => "skipped_local_changes",
            EventType::SkippedConflicts => "skipped_conflicts",
            EventType::SkippedAheadOfRemote => "skipped_ahead_of_remote",
            EventType::SyncError => "sync_error",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cloned" => Some(EventType::Cloned),
            "pulled" => Some(EventType::Pulled),
            "branch_switch" => Some(EventType::BranchSwitch),
            "skipped_local_changes" => Some(EventType::SkippedLocalChanges),
            "skipped_conflicts" => Some(EventType::SkippedConflicts),
            "skipped_ahead_of_remote" => Some(EventType::SkippedAheadOfRemote),
            "sync_error" => Some(EventType::SyncError),
            _ => None,
        }
    }

    /// Get the severity level for this event type
    pub fn severity(&self) -> Severity {
        match self {
            EventType::Cloned => Severity::Info,
            EventType::Pulled => Severity::Info,
            EventType::BranchSwitch => Severity::Warning,
            EventType::SkippedLocalChanges => Severity::Warning,
            EventType::SkippedConflicts => Severity::Warning,
            EventType::SkippedAheadOfRemote => Severity::Info,
            EventType::SyncError => Severity::Error,
        }
    }
}

/// Severity levels for events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "info" => Some(Severity::Info),
            "warning" => Some(Severity::Warning),
            "error" => Some(Severity::Error),
            _ => None,
        }
    }
}

/// Current status of a repository
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoStatus {
    /// Repository is synced and up to date
    Ok,
    /// Repository was skipped (see skip_reason)
    Skipped,
    /// Last sync resulted in an error
    Error,
    /// Repository has never been synced
    Unknown,
}

impl RepoStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RepoStatus::Ok => "ok",
            RepoStatus::Skipped => "skipped",
            RepoStatus::Error => "error",
            RepoStatus::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "ok" => RepoStatus::Ok,
            "skipped" => RepoStatus::Skipped,
            "error" => RepoStatus::Error,
            _ => RepoStatus::Unknown,
        }
    }
}

/// Repository state record
#[derive(Debug, Clone)]
pub struct RepoState {
    pub id: i64,
    pub full_name: String,
    pub local_path: Option<String>,
    pub current_branch: Option<String>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_sync_status: RepoStatus,
    pub skip_reason: Option<String>,
    pub updated_at: DateTime<Utc>,
}

/// A sync event record
#[derive(Debug, Clone)]
pub struct SyncEvent {
    pub id: i64,
    pub timestamp: DateTime<Utc>,
    pub repo_full_name: Option<String>,
    pub event_type: EventType,
    pub severity: Severity,
    pub summary: String,
    pub details: Option<String>,
    pub acknowledged: bool,
}

/// Builder for creating new sync events
#[derive(Debug)]
pub struct SyncEventBuilder {
    repo_full_name: Option<String>,
    event_type: EventType,
    summary: String,
    details: Option<String>,
}

impl SyncEventBuilder {
    pub fn new(event_type: EventType, summary: impl Into<String>) -> Self {
        Self {
            repo_full_name: None,
            event_type,
            summary: summary.into(),
            details: None,
        }
    }

    pub fn repo(mut self, full_name: impl Into<String>) -> Self {
        self.repo_full_name = Some(full_name.into());
        self
    }

    pub fn details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn details_json<T: serde::Serialize>(mut self, details: &T) -> Self {
        if let Ok(json) = serde_json::to_string(details) {
            self.details = Some(json);
        }
        self
    }
}

/// State database manager
pub struct StateDb {
    conn: Connection,
}

impl StateDb {
    /// Open or create the state database
    pub fn open() -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::open_at(db_path)
    }

    /// Open or create the state database at a specific path
    pub fn open_at(path: PathBuf) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create database directory")?;
        }

        let conn = Connection::open(&path)
            .with_context(|| format!("Failed to open database at {}", path.display()))?;

        let db = Self { conn };
        db.initialize()?;

        info!("State database opened at {}", path.display());
        Ok(db)
    }

    /// Open an in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("Failed to open in-memory database")?;
        let db = Self { conn };
        db.initialize()?;
        Ok(db)
    }

    /// Get the default database path
    fn get_db_path() -> Result<PathBuf> {
        let data_dir = if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
            PathBuf::from(data_home)
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".local/share")
        } else {
            PathBuf::from("/tmp")
        };

        Ok(data_dir.join("reposentry").join("state.db"))
    }

    /// Initialize the database schema
    fn initialize(&self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
                -- Repository state table
                CREATE TABLE IF NOT EXISTS repositories (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    full_name TEXT UNIQUE NOT NULL,
                    local_path TEXT,
                    current_branch TEXT,
                    last_sync_at TEXT,
                    last_sync_status TEXT DEFAULT 'unknown',
                    skip_reason TEXT,
                    updated_at TEXT NOT NULL
                );

                -- Event log table
                CREATE TABLE IF NOT EXISTS events (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    timestamp TEXT NOT NULL,
                    repo_full_name TEXT,
                    event_type TEXT NOT NULL,
                    severity TEXT NOT NULL,
                    summary TEXT NOT NULL,
                    details TEXT,
                    acknowledged INTEGER DEFAULT 0,
                    created_at TEXT DEFAULT CURRENT_TIMESTAMP
                );

                -- Indexes for efficient queries
                CREATE INDEX IF NOT EXISTS idx_repos_full_name ON repositories(full_name);
                CREATE INDEX IF NOT EXISTS idx_repos_status ON repositories(last_sync_status);
                CREATE INDEX IF NOT EXISTS idx_events_unack ON events(acknowledged, timestamp);
                CREATE INDEX IF NOT EXISTS idx_events_repo ON events(repo_full_name, timestamp);
                CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type, timestamp);
                "#,
            )
            .context("Failed to initialize database schema")?;

        debug!("Database schema initialized");
        Ok(())
    }

    // =========================================================================
    // Repository State Operations
    // =========================================================================

    /// Update or insert a repository's state
    pub fn upsert_repo(
        &self,
        full_name: &str,
        local_path: Option<&str>,
        current_branch: Option<&str>,
        status: RepoStatus,
        skip_reason: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let last_sync_at = if status == RepoStatus::Ok || status == RepoStatus::Skipped {
            Some(now.clone())
        } else {
            None
        };

        self.conn
            .execute(
                r#"
                INSERT INTO repositories (full_name, local_path, current_branch, last_sync_at, last_sync_status, skip_reason, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                ON CONFLICT(full_name) DO UPDATE SET
                    local_path = COALESCE(?2, local_path),
                    current_branch = COALESCE(?3, current_branch),
                    last_sync_at = COALESCE(?4, last_sync_at),
                    last_sync_status = ?5,
                    skip_reason = ?6,
                    updated_at = ?7
                "#,
                params![
                    full_name,
                    local_path,
                    current_branch,
                    last_sync_at,
                    status.as_str(),
                    skip_reason,
                    now,
                ],
            )
            .context("Failed to upsert repository")?;

        debug!("Updated repo state: {} -> {:?}", full_name, status);
        Ok(())
    }

    /// Get a repository's current state
    pub fn get_repo(&self, full_name: &str) -> Result<Option<RepoState>> {
        let result = self
            .conn
            .query_row(
                r#"
                SELECT id, full_name, local_path, current_branch, last_sync_at, last_sync_status, skip_reason, updated_at
                FROM repositories
                WHERE full_name = ?1
                "#,
                params![full_name],
                |row| {
                    Ok(RepoState {
                        id: row.get(0)?,
                        full_name: row.get(1)?,
                        local_path: row.get(2)?,
                        current_branch: row.get(3)?,
                        last_sync_at: row
                            .get::<_, Option<String>>(4)?
                            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&Utc)),
                        last_sync_status: RepoStatus::parse(
                            &row.get::<_, String>(5).unwrap_or_default(),
                        ),
                        skip_reason: row.get(6)?,
                        updated_at: row
                            .get::<_, String>(7)
                            .ok()
                            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(Utc::now),
                    })
                },
            )
            .optional()
            .context("Failed to query repository")?;

        Ok(result)
    }

    /// Get all repositories with a specific status
    pub fn get_repos_by_status(&self, status: RepoStatus) -> Result<Vec<RepoState>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, full_name, local_path, current_branch, last_sync_at, last_sync_status, skip_reason, updated_at
            FROM repositories
            WHERE last_sync_status = ?1
            ORDER BY updated_at DESC
            "#,
        )?;

        let repos = stmt
            .query_map(params![status.as_str()], |row| {
                Ok(RepoState {
                    id: row.get(0)?,
                    full_name: row.get(1)?,
                    local_path: row.get(2)?,
                    current_branch: row.get(3)?,
                    last_sync_at: row
                        .get::<_, Option<String>>(4)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    last_sync_status: RepoStatus::parse(
                        &row.get::<_, String>(5).unwrap_or_default(),
                    ),
                    skip_reason: row.get(6)?,
                    updated_at: row
                        .get::<_, String>(7)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                })
            })
            .context("Failed to query repositories")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect repositories")?;

        Ok(repos)
    }

    /// Get repositories that have issues (skipped or error)
    pub fn get_repos_with_issues(&self) -> Result<Vec<RepoState>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, full_name, local_path, current_branch, last_sync_at, last_sync_status, skip_reason, updated_at
            FROM repositories
            WHERE last_sync_status IN ('skipped', 'error')
            ORDER BY updated_at DESC
            "#,
        )?;

        let repos = stmt
            .query_map([], |row| {
                Ok(RepoState {
                    id: row.get(0)?,
                    full_name: row.get(1)?,
                    local_path: row.get(2)?,
                    current_branch: row.get(3)?,
                    last_sync_at: row
                        .get::<_, Option<String>>(4)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                    last_sync_status: RepoStatus::parse(
                        &row.get::<_, String>(5).unwrap_or_default(),
                    ),
                    skip_reason: row.get(6)?,
                    updated_at: row
                        .get::<_, String>(7)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                })
            })
            .context("Failed to query repositories with issues")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect repositories")?;

        Ok(repos)
    }

    // =========================================================================
    // Event Operations
    // =========================================================================

    /// Record a new sync event
    pub fn record_event(&self, builder: SyncEventBuilder) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        let severity = builder.event_type.severity();

        self.conn
            .execute(
                r#"
                INSERT INTO events (timestamp, repo_full_name, event_type, severity, summary, details)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                "#,
                params![
                    now,
                    builder.repo_full_name,
                    builder.event_type.as_str(),
                    severity.as_str(),
                    builder.summary,
                    builder.details,
                ],
            )
            .context("Failed to record event")?;

        let id = self.conn.last_insert_rowid();
        debug!(
            "Recorded event: {} - {}",
            builder.event_type.as_str(),
            builder.summary
        );
        Ok(id)
    }

    /// Get unacknowledged events
    pub fn get_unacknowledged_events(&self) -> Result<Vec<SyncEvent>> {
        self.get_events_with_filter(Some(false), None, None)
    }

    /// Get recent events with optional filters
    pub fn get_events_with_filter(
        &self,
        acknowledged: Option<bool>,
        event_type: Option<EventType>,
        limit: Option<u32>,
    ) -> Result<Vec<SyncEvent>> {
        let mut conditions = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ack) = acknowledged {
            conditions.push(format!("acknowledged = ?{}", param_values.len() + 1));
            param_values.push(Box::new(if ack { 1i32 } else { 0i32 }));
        }
        if let Some(et) = event_type {
            conditions.push(format!("event_type = ?{}", param_values.len() + 1));
            param_values.push(Box::new(et.as_str().to_string()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let limit_clause = limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default();

        let sql = format!(
            r#"
            SELECT id, timestamp, repo_full_name, event_type, severity, summary, details, acknowledged
            FROM events
            {}
            ORDER BY timestamp DESC
            {}
            "#,
            where_clause, limit_clause
        );

        let mut stmt = self.conn.prepare(&sql)?;

        let param_refs: Vec<&dyn rusqlite::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let events = stmt
            .query_map(param_refs.as_slice(), |row| {
                Ok(SyncEvent {
                    id: row.get(0)?,
                    timestamp: row
                        .get::<_, String>(1)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                    repo_full_name: row.get(2)?,
                    event_type: EventType::parse(&row.get::<_, String>(3)?)
                        .unwrap_or(EventType::SyncError),
                    severity: Severity::parse(&row.get::<_, String>(4)?).unwrap_or(Severity::Info),
                    summary: row.get(5)?,
                    details: row.get(6)?,
                    acknowledged: row.get::<_, i32>(7)? != 0,
                })
            })
            .context("Failed to query events")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect events")?;

        Ok(events)
    }

    /// Get events for a specific repository
    pub fn get_events_for_repo(
        &self,
        repo_full_name: &str,
        limit: Option<u32>,
    ) -> Result<Vec<SyncEvent>> {
        let limit_clause = limit.map(|l| format!(" LIMIT {}", l)).unwrap_or_default();
        let sql = format!(
            r#"
            SELECT id, timestamp, repo_full_name, event_type, severity, summary, details, acknowledged
            FROM events
            WHERE repo_full_name = ?1
            ORDER BY timestamp DESC
            {}
            "#,
            limit_clause
        );

        let mut stmt = self.conn.prepare(&sql)?;

        let events = stmt
            .query_map(params![repo_full_name], |row| {
                Ok(SyncEvent {
                    id: row.get(0)?,
                    timestamp: row
                        .get::<_, String>(1)
                        .ok()
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(Utc::now),
                    repo_full_name: row.get(2)?,
                    event_type: EventType::parse(&row.get::<_, String>(3)?)
                        .unwrap_or(EventType::SyncError),
                    severity: Severity::parse(&row.get::<_, String>(4)?).unwrap_or(Severity::Info),
                    summary: row.get(5)?,
                    details: row.get(6)?,
                    acknowledged: row.get::<_, i32>(7)? != 0,
                })
            })
            .context("Failed to query events for repo")?
            .collect::<Result<Vec<_>, _>>()
            .context("Failed to collect events")?;

        Ok(events)
    }

    /// Acknowledge an event by ID
    pub fn acknowledge_event(&self, event_id: i64) -> Result<()> {
        self.conn
            .execute(
                "UPDATE events SET acknowledged = 1 WHERE id = ?1",
                params![event_id],
            )
            .context("Failed to acknowledge event")?;
        Ok(())
    }

    /// Acknowledge all events
    pub fn acknowledge_all_events(&self) -> Result<u64> {
        let count = self
            .conn
            .execute(
                "UPDATE events SET acknowledged = 1 WHERE acknowledged = 0",
                [],
            )
            .context("Failed to acknowledge all events")?;
        Ok(count as u64)
    }

    /// Get count of unacknowledged events by severity
    pub fn get_unacknowledged_counts(&self) -> Result<(u32, u32, u32)> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT severity, COUNT(*) as count
            FROM events
            WHERE acknowledged = 0
            GROUP BY severity
            "#,
        )?;

        let mut info_count = 0u32;
        let mut warning_count = 0u32;
        let mut error_count = 0u32;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
        })?;

        for row in rows {
            let (severity, count) = row?;
            match severity.as_str() {
                "info" => info_count = count,
                "warning" => warning_count = count,
                "error" => error_count = count,
                _ => {}
            }
        }

        Ok((info_count, warning_count, error_count))
    }

    /// Clean up old events (keep last N days)
    pub fn cleanup_old_events(&self, days: u32) -> Result<u64> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        let count = self
            .conn
            .execute(
                "DELETE FROM events WHERE timestamp < ?1 AND acknowledged = 1",
                params![cutoff.to_rfc3339()],
            )
            .context("Failed to cleanup old events")?;
        Ok(count as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_initialization() {
        let db = StateDb::open_in_memory().unwrap();
        // Should not panic, tables should exist
        let count: i32 = db
            .conn
            .query_row("SELECT COUNT(*) FROM repositories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_repo_upsert_and_get() {
        let db = StateDb::open_in_memory().unwrap();

        db.upsert_repo(
            "owner/repo",
            Some("/path/to/repo"),
            Some("main"),
            RepoStatus::Ok,
            None,
        )
        .unwrap();

        let repo = db.get_repo("owner/repo").unwrap().unwrap();
        assert_eq!(repo.full_name, "owner/repo");
        assert_eq!(repo.local_path, Some("/path/to/repo".to_string()));
        assert_eq!(repo.current_branch, Some("main".to_string()));
        assert_eq!(repo.last_sync_status, RepoStatus::Ok);
    }

    #[test]
    fn test_repo_update() {
        let db = StateDb::open_in_memory().unwrap();

        // Initial insert
        db.upsert_repo(
            "owner/repo",
            Some("/path"),
            Some("main"),
            RepoStatus::Ok,
            None,
        )
        .unwrap();

        // Update with skip
        db.upsert_repo(
            "owner/repo",
            None, // Don't change path
            Some("dev"),
            RepoStatus::Skipped,
            Some("local changes"),
        )
        .unwrap();

        let repo = db.get_repo("owner/repo").unwrap().unwrap();
        assert_eq!(repo.local_path, Some("/path".to_string())); // Preserved
        assert_eq!(repo.current_branch, Some("dev".to_string())); // Updated
        assert_eq!(repo.last_sync_status, RepoStatus::Skipped);
        assert_eq!(repo.skip_reason, Some("local changes".to_string()));
    }

    #[test]
    fn test_record_and_get_events() {
        let db = StateDb::open_in_memory().unwrap();

        let id = db
            .record_event(
                SyncEventBuilder::new(EventType::BranchSwitch, "Switched from main to dev")
                    .repo("owner/repo")
                    .details("dev is 10 commits ahead"),
            )
            .unwrap();

        assert!(id > 0);

        let events = db.get_unacknowledged_events().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, EventType::BranchSwitch);
        assert_eq!(events[0].severity, Severity::Warning);
        assert!(!events[0].acknowledged);
    }

    #[test]
    fn test_acknowledge_events() {
        let db = StateDb::open_in_memory().unwrap();

        db.record_event(SyncEventBuilder::new(EventType::Cloned, "Cloned repo").repo("owner/repo"))
            .unwrap();

        let events = db.get_unacknowledged_events().unwrap();
        assert_eq!(events.len(), 1);

        db.acknowledge_event(events[0].id).unwrap();

        let events = db.get_unacknowledged_events().unwrap();
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_unacknowledged_counts() {
        let db = StateDb::open_in_memory().unwrap();

        // Add events of different severities
        db.record_event(SyncEventBuilder::new(EventType::Cloned, "Cloned"))
            .unwrap(); // Info
        db.record_event(SyncEventBuilder::new(EventType::BranchSwitch, "Switched"))
            .unwrap(); // Warning
        db.record_event(SyncEventBuilder::new(
            EventType::SkippedLocalChanges,
            "Skipped",
        ))
        .unwrap(); // Warning
        db.record_event(SyncEventBuilder::new(EventType::SyncError, "Error"))
            .unwrap(); // Error

        let (info, warning, error) = db.get_unacknowledged_counts().unwrap();
        assert_eq!(info, 1);
        assert_eq!(warning, 2);
        assert_eq!(error, 1);
    }

    #[test]
    fn test_repos_with_issues() {
        let db = StateDb::open_in_memory().unwrap();

        db.upsert_repo("owner/ok-repo", None, None, RepoStatus::Ok, None)
            .unwrap();
        db.upsert_repo(
            "owner/skipped-repo",
            None,
            None,
            RepoStatus::Skipped,
            Some("local changes"),
        )
        .unwrap();
        db.upsert_repo(
            "owner/error-repo",
            None,
            None,
            RepoStatus::Error,
            Some("network error"),
        )
        .unwrap();

        let issues = db.get_repos_with_issues().unwrap();
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn test_event_type_severity() {
        assert_eq!(EventType::Cloned.severity(), Severity::Info);
        assert_eq!(EventType::Pulled.severity(), Severity::Info);
        assert_eq!(EventType::BranchSwitch.severity(), Severity::Warning);
        assert_eq!(EventType::SkippedLocalChanges.severity(), Severity::Warning);
        assert_eq!(EventType::SyncError.severity(), Severity::Error);
    }
}
