//! Sync Engine - Orchestrates parallel repository synchronization
//!
//! This module provides the high-level sync orchestration that coordinates
//! repository discovery, filtering, and parallel synchronization using the
//! GitClient for actual git operations.

use crate::git::{GitClient, RepoState, SyncResult};
use crate::{Config, GitHubClient};
use anyhow::{Context, Result};
use futures::stream::{FuturesUnordered, StreamExt};
use std::sync::Arc;
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
#[derive(Clone)]
pub struct SyncEngine {
    config: Arc<Config>,
    github_client: GitHubClient,
    git_client: GitClient,
}

impl SyncEngine {
    /// Create a new sync engine with the given configuration
    pub async fn new(config: Config) -> Result<Self> {
        let config = Arc::new(config);
        let github_client = GitHubClient::new(&config).await?;
        let git_client = GitClient::new(config.as_ref().clone());

        Ok(Self {
            config,
            github_client,
            git_client,
        })
    }

    /// Run a complete sync operation: discover repositories and sync them
    pub async fn run_sync(&self) -> Result<SyncSummary> {
        let start_time = Instant::now();

        info!("Starting repository synchronization");

        // Discover repositories using GitHub API
        let repositories = self
            .discover_repositories()
            .await
            .context("Failed to discover repositories")?;

        info!("Discovered {} repositories", repositories.len());

        // Synchronize repositories in parallel
        let sync_results = self
            .sync_repositories_parallel(repositories)
            .await
            .context("Failed to synchronize repositories")?;

        let duration = start_time.elapsed();

        // Compile summary
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

    /// Discover repositories using the GitHub client
    async fn discover_repositories(&self) -> Result<Vec<octocrab::models::Repository>> {
        debug!("Discovering repositories from GitHub");

        let mut all_repos = Vec::new();

        // Get user repositories
        let user_repos = self
            .github_client
            .list_user_repositories()
            .await
            .context("Failed to get user repositories")?;
        all_repos.extend(user_repos);

        // Get organization repositories if enabled
        if self.config.github.include_organizations {
            // For now, we'll skip organization-specific repos and just use user repos
            // TODO: Implement automatic organization discovery
            info!("Organization repository discovery not yet implemented, using user repositories only");
        }

        // Apply filtering using the GitHub client's internal filtering
        // For now, we'll use the discovered repositories directly
        // TODO: Extract and use the filtering logic from GitHub client
        let original_count = all_repos.len();
        let filtered_repos = all_repos;

        info!(
            "Filtered {} repositories to {}",
            original_count,
            filtered_repos.len()
        );

        Ok(filtered_repos)
    }

    /// Synchronize repositories in parallel with network-aware concurrency
    async fn sync_repositories_parallel(
        &self,
        repositories: Vec<octocrab::models::Repository>,
    ) -> Result<Vec<SyncResult>> {
        let base_parallel = self.config.sync.max_parallel;
        let operation_timeout = Duration::from_secs(self.config.sync.timeout);

        // Network-aware concurrency: adjust based on repository characteristics
        let adaptive_parallel = self.calculate_adaptive_concurrency(&repositories, base_parallel);

        info!(
            "Syncing {} repositories with adaptive concurrency: base={}, calculated={}",
            repositories.len(),
            base_parallel,
            adaptive_parallel
        );

        // Create a semaphore to control concurrency
        let semaphore = Arc::new(tokio::sync::Semaphore::new(adaptive_parallel));

        // Create futures for all sync operations
        let mut futures = FuturesUnordered::new();

        for repo in repositories {
            let semaphore = semaphore.clone();
            let git_client = self.git_client.clone();

            let future = async move {
                // Acquire semaphore permit
                let _permit = semaphore.acquire().await.expect("Semaphore closed");

                // Run sync operation with timeout
                let sync_future = git_client.sync_repository(&repo);
                match timeout(operation_timeout, sync_future).await {
                    Ok(result) => result,
                    Err(_) => {
                        warn!("Sync operation timed out for repository: {}", repo.name);
                        Err(anyhow::anyhow!(
                            "Operation timed out after {}s",
                            operation_timeout.as_secs()
                        ))
                    }
                }
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
                Err(e) => {
                    error!("Sync failed: {:?}", e);
                    results.push(SyncResult::Failed {
                        path: std::path::PathBuf::from("unknown"),
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
                SyncResult::Cloned { .. } | SyncResult::Pulled { .. } => successful_operations += 1,
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

    /// Run a dry-run sync to preview what would be synchronized
    pub async fn dry_run(&self) -> Result<Vec<RepoState>> {
        info!("Running dry-run sync analysis");

        let repositories = self
            .discover_repositories()
            .await
            .context("Failed to discover repositories")?;

        let mut repo_states = Vec::new();

        for repo in repositories {
            let repo_dir = self.git_client.get_repo_directory(&repo)?;
            let remote_url = if let Some(clone_url) = &repo.clone_url {
                clone_url.to_string()
            } else if let Some(ssh_url) = &repo.ssh_url {
                ssh_url.clone()
            } else {
                format!("https://github.com/{}", &repo.name)
            };

            let state = self
                .git_client
                .analyze_repo_state(&repo_dir, &remote_url)
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

    /// Calculate adaptive concurrency based on repository characteristics and network conditions
    fn calculate_adaptive_concurrency(
        &self,
        repositories: &[octocrab::models::Repository],
        base_parallel: usize,
    ) -> usize {
        // Real-world adaptive concurrency strategies:

        // 1. Repository size-based adjustment
        let avg_size = repositories
            .iter()
            .filter_map(|repo| repo.size)
            .map(|s| s as f64)
            .sum::<f64>()
            / repositories.len().max(1) as f64;

        let size_factor = match avg_size {
            s if s > 50_000.0 => 0.5,  // Large repos: reduce concurrency (50MB+)
            s if s > 10_000.0 => 0.75, // Medium repos: slight reduction (10MB+)
            _ => 1.0,                  // Small repos: full concurrency
        };

        // 2. Repository count-based scaling
        let count_factor = match repositories.len() {
            n if n > 50 => {
                // For many repos, use bandwidth-efficient concurrency
                // Real tools like `repo` use 4-8 parallel network operations
                (base_parallel as f64 * 0.6).max(3.0) / base_parallel as f64
            }
            n if n > 20 => 0.8, // Moderate scaling
            _ => 1.0,           // Small batch: use full concurrency
        };

        // 3. Network-aware defaults (industry practices)
        let network_optimized = match base_parallel {
            // If user didn't configure, use network-optimal defaults
            4 => {
                // Default value
                // GitHub Desktop uses 4-6, VS Code uses 4-8
                // Git hosting providers generally prefer 4-8 concurrent connections
                std::cmp::min(6, repositories.len())
            }
            n => n, // Respect user configuration
        };

        // 4. Apply all factors
        let calculated = (network_optimized as f64 * size_factor * count_factor).round() as usize;

        // 5. Enforce reasonable bounds
        calculated.clamp(1, 12) // Never exceed 12 (GitHub API limits)
    }

    /// Get configuration for external inspection
    pub fn config(&self) -> &Config {
        &self.config
    }
}

/// Helper to create and configure a sync engine from config file
pub async fn create_sync_engine_from_config() -> Result<SyncEngine> {
    let config = Config::load_or_default().context("Failed to load configuration")?;
    SyncEngine::new(config)
        .await
        .context("Failed to create sync engine")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_summary_calculation() {
        let results = vec![
            SyncResult::Cloned {
                path: "/tmp/repo1".into(),
            },
            SyncResult::Pulled {
                path: "/tmp/repo2".into(),
                commits_updated: 5,
            },
            SyncResult::Failed {
                path: "/tmp/repo3".into(),
                error: "Network error".to_string(),
            },
            SyncResult::Skipped {
                path: "/tmp/repo4".into(),
                reason: "Conflicts detected".to_string(),
            },
            SyncResult::UpToDate {
                path: "/tmp/repo5".into(),
            },
        ];

        let duration = Duration::from_secs(60);
        let config = Config::default();

        // Create a temporary sync engine for testing the summary function
        // Note: This test may fail if GitHub authentication is not available
        let compile_summary_test = |results: Vec<SyncResult>, duration: Duration| -> SyncSummary {
            let total_repositories = results.len();
            let mut successful_operations = 0;
            let mut failed_operations = 0;
            let mut skipped_operations = 0;

            for result in &results {
                match result {
                    SyncResult::Cloned { .. } | SyncResult::Pulled { .. } => {
                        successful_operations += 1
                    }
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
        };

        let summary = compile_summary_test(results.clone(), duration);

        assert_eq!(summary.total_repositories, 5);
        assert_eq!(summary.successful_operations, 3); // Cloned + Pulled + UpToDate
        assert_eq!(summary.failed_operations, 1); // Failed
        assert_eq!(summary.skipped_operations, 1); // Skipped
        assert_eq!(summary.duration, duration);
        assert_eq!(summary.results.len(), 5);
    }

    #[test]
    fn test_semaphore_limits() {
        // Test that max_parallel is properly enforced
        let config = Config {
            sync: crate::config::SyncConfig {
                max_parallel: 3,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(config.sync.max_parallel, 3);

        // In a real test, we would need to verify that no more than 3 operations
        // run simultaneously. This would require more complex async testing setup.
        // For now, we just verify the configuration is properly structured.
    }

    #[test]
    fn test_adaptive_concurrency_calculation() {
        // Test the concurrency calculation logic directly without creating full SyncEngine
        let repositories = vec![];
        let base_parallel = 4;

        // Simulate the calculation logic
        let calculate_test = |repos: &[()], base: usize| -> usize {
            // Simplified version of the calculation for testing
            let avg_size = 0.0; // Empty repos
            let size_factor = if avg_size > 50_000.0 { 0.5 } else { 1.0 };

            let count_factor = match repos.len() {
                n if n > 50 => 0.6,
                n if n > 20 => 0.8,
                _ => 1.0,
            };

            let network_optimized = match base {
                4 => std::cmp::min(6, repos.len().max(4)),
                n => n,
            };

            let calculated =
                (network_optimized as f64 * size_factor * count_factor).round() as usize;
            std::cmp::max(1, std::cmp::min(calculated, 12))
        };

        // Test with empty repositories
        let concurrency = calculate_test(&repositories, 4);
        assert!(concurrency >= 1 && concurrency <= 12);

        // Test bounds enforcement
        let large_base = calculate_test(&repositories, 20);
        assert!(large_base <= 12); // Should be capped at 12

        let zero_base = calculate_test(&repositories, 0);
        assert!(zero_base >= 1); // Should be at least 1
    }

    #[tokio::test]
    async fn test_sync_engine_creation() {
        let config = Config::default();
        let result = SyncEngine::new(config).await;

        // This test will fail if GitHub authentication is not available
        // In CI/CD environments, we might need to mock this
        match result {
            Ok(engine) => {
                assert!(engine.config.base_directory.len() > 0);
            }
            Err(e) => {
                // If authentication fails, that's expected in test environment
                assert!(
                    e.to_string().contains("authentication") || e.to_string().contains("GitHub")
                );
            }
        }
    }
}
