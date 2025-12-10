//! Sync Engine - Orchestrates parallel repository synchronization
//!
//! This module provides the high-level sync orchestration that coordinates
//! parallel synchronization using GitClient for actual git operations.
//!
//! The SyncEngine is provider-agnostic - it works with `RepoSpec` objects
//! that can come from any discovery source (GitHub, GitLab, local, etc.)

use crate::discovery::RepoSpec;
use crate::git::{GitClient, RepoState, SyncResult};
use crate::state::{EventType, RepoStatus, StateDb, SyncEventBuilder};
use crate::Config;
use anyhow::{Context, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Results from a complete sync operation
#[derive(Debug, Clone)]
pub struct SyncSummary {
    pub total_repositories: usize,
    pub successful_operations: usize,
    pub failed_operations: usize,
    pub skipped_operations: usize,
    pub duration: Duration,
    pub results: Vec<SyncResult>,
}

/// The main sync engine that orchestrates repository synchronization
///
/// SyncEngine is provider-agnostic. It accepts `Vec<RepoSpec>` from any
/// discovery source and performs git operations using GitClient.
#[derive(Clone)]
pub struct SyncEngine {
    config: Arc<Config>,
    git_client: GitClient,
    state_db: Option<Arc<Mutex<StateDb>>>,
}

impl SyncEngine {
    /// Create a new sync engine with the given configuration
    pub fn new(config: Config) -> Self {
        let config = Arc::new(config);
        let git_client = GitClient::new(config.as_ref().clone());

        Self {
            config,
            git_client,
            state_db: None,
        }
    }

    /// Create a sync engine with state database for event tracking
    pub fn with_state_db(config: Config) -> Result<Self> {
        let config = Arc::new(config);
        let git_client = GitClient::new(config.as_ref().clone());
        let state_db = StateDb::open().context("Failed to open state database")?;

        Ok(Self {
            config,
            git_client,
            state_db: Some(Arc::new(Mutex::new(state_db))),
        })
    }

    /// Create a sync engine with a custom state database (for testing)
    pub fn with_custom_state_db(config: Config, state_db: StateDb) -> Self {
        let config = Arc::new(config);
        let git_client = GitClient::new(config.as_ref().clone());

        Self {
            config,
            git_client,
            state_db: Some(Arc::new(Mutex::new(state_db))),
        }
    }

    /// Sync repositories from RepoSpec list (provider-agnostic)
    ///
    /// This is the primary sync method. It accepts pre-discovered repositories
    /// as `Vec<RepoSpec>` and performs parallel synchronization.
    ///
    /// If a state database is configured, sync results are automatically recorded.
    pub async fn sync_repos(&self, repos: Vec<RepoSpec>) -> Result<SyncSummary> {
        let start_time = Instant::now();

        info!("Starting synchronization of {} repositories", repos.len());

        let sync_results = self
            .sync_specs_parallel(repos)
            .await
            .context("Failed to synchronize repositories")?;

        // Record results to state database if configured
        self.record_sync_results(&sync_results);

        let duration = start_time.elapsed();
        let summary = self.compile_summary(sync_results, duration);

        info!(
            "Sync completed in {:.2}s: {} successful, {} failed, {} skipped",
            summary.duration.as_secs_f64(),
            summary.successful_operations,
            summary.failed_operations,
            summary.skipped_operations
        );

        Ok(summary)
    }

    /// Analyze repositories without syncing (dry-run)
    ///
    /// Returns the current state of each repository for preview.
    pub async fn analyze_repos(&self, repos: &[RepoSpec]) -> Result<Vec<RepoState>> {
        info!("Running dry-run analysis for {} repositories", repos.len());

        let mut repo_states = Vec::new();

        for spec in repos {
            let state = self
                .git_client
                .analyze_from_spec(spec)
                .await
                .context("Failed to analyze repository state")?;
            repo_states.push(state);
        }

        info!(
            "Dry-run analysis completed for {} repositories",
            repo_states.len()
        );

        Ok(repo_states)
    }

    /// Synchronize repositories in parallel with network-aware concurrency
    async fn sync_specs_parallel(&self, repos: Vec<RepoSpec>) -> Result<Vec<SyncResult>> {
        let base_parallel = self.config.sync.max_parallel;
        let operation_timeout = Duration::from_secs(self.config.sync.timeout);

        // Network-aware concurrency: adjust based on repository characteristics
        let adaptive_parallel = self.calculate_adaptive_concurrency(&repos, base_parallel);

        info!(
            "Syncing {} repositories with adaptive concurrency: base={}, calculated={}",
            repos.len(),
            base_parallel,
            adaptive_parallel
        );

        // Create a semaphore to control concurrency
        let semaphore = Arc::new(tokio::sync::Semaphore::new(adaptive_parallel));

        // Create futures for all sync operations
        let mut futures = FuturesUnordered::new();

        for spec in repos {
            let semaphore = semaphore.clone();
            let git_client = self.git_client.clone();

            let future = async move {
                // Acquire semaphore permit
                let _permit = semaphore.acquire().await.expect("Semaphore closed");

                let spec_name = spec.full_name();
                let spec_path = spec.local_path.clone();

                // Run sync operation with timeout
                let sync_future = git_client.sync_from_spec(&spec);
                match timeout(operation_timeout, sync_future).await {
                    Ok(result) => result,
                    Err(_) => {
                        warn!("Sync operation timed out for repository: {}", spec_name);
                        Err(anyhow::anyhow!(
                            "Operation timed out after {}s",
                            operation_timeout.as_secs()
                        ))
                    }
                }
                .map_err(|e| (spec_path, e))
            };

            futures.push(future);
        }

        // Collect all results
        let mut results = Vec::new();

        while let Some(result) = futures.next().await {
            match result {
                Ok(sync_result) => {
                    debug!("Sync completed: {:?}", sync_result);
                    results.push(sync_result);
                }
                Err((path, e)) => {
                    error!("Sync failed for {}: {:?}", path.display(), e);
                    results.push(SyncResult::Failed {
                        path,
                        error: format!("Sync operation failed: {}", e),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Compile sync summary from results
    fn compile_summary(&self, results: Vec<SyncResult>, duration: Duration) -> SyncSummary {
        let total_repositories = results.len();
        let mut successful_operations = 0;
        let mut failed_operations = 0;
        let mut skipped_operations = 0;

        for result in &results {
            match result {
                SyncResult::Cloned { .. }
                | SyncResult::Pulled { .. }
                | SyncResult::BranchSwitched { .. } => successful_operations += 1,
                SyncResult::FetchedOnly { .. } | SyncResult::UpToDate { .. } => {
                    successful_operations += 1
                }
                SyncResult::Skipped { .. } => skipped_operations += 1,
                SyncResult::Failed { .. } => failed_operations += 1,
            }
        }

        SyncSummary {
            total_repositories,
            successful_operations,
            failed_operations,
            skipped_operations,
            duration,
            results,
        }
    }

    /// Calculate adaptive concurrency based on repository characteristics
    fn calculate_adaptive_concurrency(&self, repos: &[RepoSpec], base_parallel: usize) -> usize {
        // 1. Repository size-based adjustment
        let avg_size = repos
            .iter()
            .filter_map(|r| r.size_bytes)
            .map(|s| s as f64)
            .sum::<f64>()
            / repos.len().max(1) as f64;

        let size_factor = match avg_size {
            s if s > 50_000_000.0 => 0.5, // Large repos: reduce concurrency (50MB+)
            s if s > 10_000_000.0 => 0.75, // Medium repos: slight reduction (10MB+)
            _ => 1.0,                     // Small repos: full concurrency
        };

        // 2. Repository count-based scaling
        let count_factor = match repos.len() {
            n if n > 50 => {
                // For many repos, use bandwidth-efficient concurrency
                (base_parallel as f64 * 0.6).max(3.0) / base_parallel as f64
            }
            n if n > 20 => 0.8,
            _ => 1.0,
        };

        // 3. Network-aware defaults
        let network_optimized = match base_parallel {
            4 => std::cmp::min(6, repos.len()),
            n => n,
        };

        // 4. Apply all factors
        let calculated = (network_optimized as f64 * size_factor * count_factor).round() as usize;

        // 5. Enforce reasonable bounds
        calculated.clamp(1, 12)
    }

    /// Get configuration for external inspection
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get the git client for direct operations
    pub fn git_client(&self) -> &GitClient {
        &self.git_client
    }

    /// Get the state database (if configured)
    pub fn state_db(&self) -> Option<&Arc<Mutex<StateDb>>> {
        self.state_db.as_ref()
    }

    /// Record a sync result to the state database
    fn record_sync_result(&self, result: &SyncResult, repo_full_name: &str) {
        let Some(state_db) = &self.state_db else {
            return;
        };

        let Ok(db) = state_db.lock() else {
            warn!("Failed to acquire state database lock");
            return;
        };

        // Record repo state and event based on result
        match result {
            SyncResult::Cloned { path, branch } => {
                let branch_ref = branch.as_deref();
                if let Err(e) = db.upsert_repo(
                    repo_full_name,
                    Some(&path.to_string_lossy()),
                    branch_ref,
                    RepoStatus::Ok,
                    None,
                ) {
                    warn!("Failed to update repo state: {}", e);
                }

                let summary = format!(
                    "Cloned {}",
                    branch_ref.map(|b| format!(" on branch {}", b)).unwrap_or_default()
                );
                if let Err(e) = db.record_event(
                    SyncEventBuilder::new(EventType::Cloned, summary).repo(repo_full_name),
                ) {
                    warn!("Failed to record clone event: {}", e);
                }
            }

            SyncResult::Pulled {
                path,
                commits_updated,
                branch,
            } => {
                let branch_ref = branch.as_deref();
                if let Err(e) = db.upsert_repo(
                    repo_full_name,
                    Some(&path.to_string_lossy()),
                    branch_ref,
                    RepoStatus::Ok,
                    None,
                ) {
                    warn!("Failed to update repo state: {}", e);
                }

                // Only record pull event if commits were updated (not just up to date)
                if *commits_updated > 0 {
                    let summary = format!("Pulled {} commits", commits_updated);
                    if let Err(e) = db.record_event(
                        SyncEventBuilder::new(EventType::Pulled, summary).repo(repo_full_name),
                    ) {
                        warn!("Failed to record pull event: {}", e);
                    }
                }
            }

            SyncResult::BranchSwitched {
                path,
                from_branch,
                to_branch,
                commits_updated,
            } => {
                if let Err(e) = db.upsert_repo(
                    repo_full_name,
                    Some(&path.to_string_lossy()),
                    Some(to_branch),
                    RepoStatus::Ok,
                    None,
                ) {
                    warn!("Failed to update repo state: {}", e);
                }

                let summary = format!(
                    "Switched from {} to {} ({} commits)",
                    from_branch, to_branch, commits_updated
                );
                if let Err(e) = db.record_event(
                    SyncEventBuilder::new(EventType::BranchSwitch, summary)
                        .repo(repo_full_name)
                        .details(format!(
                            "{{\"from\": \"{}\", \"to\": \"{}\", \"commits\": {}}}",
                            from_branch, to_branch, commits_updated
                        )),
                ) {
                    warn!("Failed to record branch switch event: {}", e);
                }
            }

            SyncResult::FetchedOnly { path, reason } => {
                // Determine if this is due to local changes or conflicts
                let (event_type, status) = if reason.contains("local changes") {
                    (EventType::SkippedLocalChanges, RepoStatus::Skipped)
                } else if reason.contains("conflict") {
                    (EventType::SkippedConflicts, RepoStatus::Skipped)
                } else if reason.contains("ahead") {
                    (EventType::SkippedAheadOfRemote, RepoStatus::Skipped)
                } else {
                    // Generic fetch-only, still mark as OK since fetch succeeded
                    (EventType::Pulled, RepoStatus::Ok)
                };

                if let Err(e) = db.upsert_repo(
                    repo_full_name,
                    Some(&path.to_string_lossy()),
                    None,
                    status,
                    Some(reason),
                ) {
                    warn!("Failed to update repo state: {}", e);
                }

                // Only record event if it was a skip (not a normal fetch)
                if event_type != EventType::Pulled {
                    let summary = format!("Fetch only: {}", reason);
                    if let Err(e) = db.record_event(
                        SyncEventBuilder::new(event_type, summary).repo(repo_full_name),
                    ) {
                        warn!("Failed to record fetch-only event: {}", e);
                    }
                }
            }

            SyncResult::UpToDate { path, branch } => {
                if let Err(e) = db.upsert_repo(
                    repo_full_name,
                    Some(&path.to_string_lossy()),
                    branch.as_deref(),
                    RepoStatus::Ok,
                    None,
                ) {
                    warn!("Failed to update repo state: {}", e);
                }
                // Don't record event for up-to-date repos (too noisy)
            }

            SyncResult::Skipped { path, reason } => {
                // Determine skip type for proper categorization
                let event_type = if reason.contains("local changes") {
                    EventType::SkippedLocalChanges
                } else if reason.contains("conflict") {
                    EventType::SkippedConflicts
                } else if reason.contains("ahead") {
                    EventType::SkippedAheadOfRemote
                } else {
                    EventType::SkippedLocalChanges // Default to local changes
                };

                if let Err(e) = db.upsert_repo(
                    repo_full_name,
                    Some(&path.to_string_lossy()),
                    None,
                    RepoStatus::Skipped,
                    Some(reason),
                ) {
                    warn!("Failed to update repo state: {}", e);
                }

                let summary = format!("Skipped: {}", reason);
                if let Err(e) = db.record_event(
                    SyncEventBuilder::new(event_type, summary).repo(repo_full_name),
                ) {
                    warn!("Failed to record skip event: {}", e);
                }
            }

            SyncResult::Failed { path, error } => {
                if let Err(e) = db.upsert_repo(
                    repo_full_name,
                    Some(&path.to_string_lossy()),
                    None,
                    RepoStatus::Error,
                    Some(error),
                ) {
                    warn!("Failed to update repo state: {}", e);
                }

                let summary = format!("Sync error: {}", error);
                if let Err(e) = db.record_event(
                    SyncEventBuilder::new(EventType::SyncError, summary).repo(repo_full_name),
                ) {
                    warn!("Failed to record error event: {}", e);
                }
            }
        }
    }

    /// Record all sync results to the state database
    pub fn record_sync_results(&self, results: &[SyncResult]) {
        for result in results {
            // Extract repo full name from the path
            let repo_name = match result {
                SyncResult::Cloned { path, .. }
                | SyncResult::Pulled { path, .. }
                | SyncResult::BranchSwitched { path, .. }
                | SyncResult::FetchedOnly { path, .. }
                | SyncResult::UpToDate { path, .. }
                | SyncResult::Skipped { path, .. }
                | SyncResult::Failed { path, .. } => {
                    // Try to extract owner/repo from path (assuming structure like /base/owner/repo or /base/repo)
                    let components: Vec<_> = path.components().rev().take(2).collect();
                    match components.as_slice() {
                        [repo, owner] => format!(
                            "{}/{}",
                            owner.as_os_str().to_string_lossy(),
                            repo.as_os_str().to_string_lossy()
                        ),
                        [repo] => repo.as_os_str().to_string_lossy().to_string(),
                        _ => path.to_string_lossy().to_string(),
                    }
                }
            };

            self.record_sync_result(result, &repo_name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_sync_summary_calculation() {
        let results = vec![
            SyncResult::Cloned {
                path: PathBuf::from("/test/repo1"),
                branch: Some("main".to_string()),
            },
            SyncResult::Pulled {
                path: PathBuf::from("/test/repo2"),
                commits_updated: 5,
                branch: Some("main".to_string()),
            },
            SyncResult::Failed {
                path: PathBuf::from("/test/repo3"),
                error: "Test error".to_string(),
            },
            SyncResult::Skipped {
                path: PathBuf::from("/test/repo4"),
                reason: "Has changes".to_string(),
            },
        ];

        let config = Config::default();
        let engine = SyncEngine::new(config);
        let summary = engine.compile_summary(results, Duration::from_secs(10));

        assert_eq!(summary.total_repositories, 4);
        assert_eq!(summary.successful_operations, 2);
        assert_eq!(summary.failed_operations, 1);
        assert_eq!(summary.skipped_operations, 1);
    }

    #[test]
    fn test_sync_engine_creation() {
        let config = Config::default();
        let engine = SyncEngine::new(config);
        assert_eq!(engine.config().sync.max_parallel, 4);
    }

    #[test]
    fn test_adaptive_concurrency_calculation() {
        let config = Config::default();
        let engine = SyncEngine::new(config);

        // Test with small repos
        let small_repos: Vec<RepoSpec> = (0..5)
            .map(|i| RepoSpec {
                name: format!("repo{}", i),
                owner: "test".to_string(),
                clone_url: format!("git@github.com:test/repo{}.git", i),
                clone_url_alt: None,
                clone_method: crate::discovery::CloneMethod::Ssh,
                local_path: PathBuf::from(format!("/test/repo{}", i)),
                is_fork: false,
                is_archived: false,
                size_bytes: Some(1024 * 1024), // 1MB
                default_branch: Some("main".to_string()),
                provider: "test".to_string(),
            })
            .collect();

        let concurrency = engine.calculate_adaptive_concurrency(&small_repos, 4);
        assert!(concurrency >= 1 && concurrency <= 12);
    }

    #[test]
    fn test_semaphore_limits() {
        let config = Config::default();
        let engine = SyncEngine::new(config);

        // Large number of large repos should reduce concurrency
        let large_repos: Vec<RepoSpec> = (0..100)
            .map(|i| RepoSpec {
                name: format!("repo{}", i),
                owner: "test".to_string(),
                clone_url: format!("git@github.com:test/repo{}.git", i),
                clone_url_alt: None,
                clone_method: crate::discovery::CloneMethod::Ssh,
                local_path: PathBuf::from(format!("/test/repo{}", i)),
                is_fork: false,
                is_archived: false,
                size_bytes: Some(100 * 1024 * 1024), // 100MB
                default_branch: Some("main".to_string()),
                provider: "test".to_string(),
            })
            .collect();

        let concurrency = engine.calculate_adaptive_concurrency(&large_repos, 4);
        assert!(concurrency <= 6); // Should be reduced for large repos
    }
}
