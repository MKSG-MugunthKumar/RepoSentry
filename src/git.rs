use anyhow::{anyhow, Context, Result};
use octocrab::models::Repository;
use std::path::{Path, PathBuf};
use tokio::process::Command as AsyncCommand;
use tracing::{debug, info, warn};

use crate::config::Config;

/// Git operations handler with intelligent conflict detection and safe synchronization
#[derive(Clone)]
pub struct GitClient {
    config: Config,
}

/// Represents the state of a git repository for sync decision making
#[derive(Debug, Clone)]
pub struct RepoState {
    pub path: PathBuf,
    pub exists: bool,
    pub has_uncommitted_changes: bool,
    pub has_untracked_files: bool,
    pub is_ahead_of_remote: bool,
    pub is_behind_remote: bool,
    pub has_conflicts: bool,
    pub remote_url: Option<String>,
    pub current_branch: Option<String>,
}

/// Result of a sync operation
#[derive(Debug, Clone)]
pub enum SyncResult {
    /// Repository was successfully cloned
    Cloned {
        path: PathBuf,
        branch: Option<String>,
    },
    /// Repository was successfully pulled
    Pulled {
        path: PathBuf,
        commits_updated: u32,
        branch: Option<String>,
    },
    /// Branch was switched to a more recent one before pull
    BranchSwitched {
        path: PathBuf,
        from_branch: String,
        to_branch: String,
        commits_updated: u32,
    },
    /// Repository was fetched but not pulled due to conflicts
    FetchedOnly { path: PathBuf, reason: String },
    /// Repository was already up to date
    UpToDate {
        path: PathBuf,
        branch: Option<String>,
    },
    /// Repository was skipped due to configuration or errors
    Skipped { path: PathBuf, reason: String },
    /// Operation failed with error
    Failed { path: PathBuf, error: String },
}

impl GitClient {
    /// Create a new Git client with the given configuration
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Get the target directory for a repository based on organization settings
    pub fn get_repo_directory(&self, repo: &Repository) -> Result<PathBuf> {
        let mut base_path = PathBuf::from(&self.config.base_directory);

        // Expand environment variables if needed
        let path_string = base_path.to_string_lossy().to_string();
        if path_string.contains('$') {
            let expanded = shellexpand::full(&path_string)?;
            base_path = PathBuf::from(expanded.as_ref());
        }

        // Get repository name safely
        let repo_name = &repo.name;
        let full_name = repo.full_name.as_deref().unwrap_or(&repo.name);

        // Create organization-based directory structure if configured
        if self.config.organization.separate_org_dirs {
            // Extract organization from repository full_name
            if let Some(slash_pos) = full_name.find('/') {
                let org = &full_name[..slash_pos];
                let repo_name = &full_name[slash_pos + 1..];

                base_path = base_path.join(org).join(repo_name);
            } else {
                base_path = base_path.join(repo_name);
            }
        } else {
            // Handle naming conflicts
            match self.config.organization.conflict_resolution.as_str() {
                "prefix-org" => {
                    base_path = base_path.join(full_name.replace('/', "-"));
                }
                "suffix" => {
                    base_path = base_path.join(repo_name);
                    // Note: Suffix handling would need additional logic to detect conflicts
                }
                _ => {
                    // "skip" and default
                    base_path = base_path.join(repo_name);
                }
            }
        }

        Ok(base_path)
    }

    /// Analyze the current state of a repository
    pub async fn analyze_repo_state(&self, path: &Path, remote_url: &str) -> Result<RepoState> {
        if !path.exists() {
            return Ok(RepoState {
                path: path.to_path_buf(),
                exists: false,
                has_uncommitted_changes: false,
                has_untracked_files: false,
                is_ahead_of_remote: false,
                is_behind_remote: false,
                has_conflicts: false,
                remote_url: Some(remote_url.to_string()),
                current_branch: None,
            });
        }

        debug!("Analyzing repository state: {}", path.display());

        // Check if it's a git repository
        let git_dir = path.join(".git");
        if !git_dir.exists() {
            return Err(anyhow!(
                "Directory exists but is not a git repository: {}",
                path.display()
            ));
        }

        let has_uncommitted_changes = self.has_uncommitted_changes(path).await?;
        let has_untracked_files = self.has_untracked_files(path).await?;
        let current_branch = self.get_current_branch(path).await?;
        let actual_remote_url = self.get_remote_url(path).await?;

        // Fetch latest remote information
        if let Err(e) = self.git_fetch(path).await {
            warn!("Failed to fetch remote for {}: {}", path.display(), e);
        }

        let is_ahead_of_remote = self.is_ahead_of_remote(path).await?;
        let is_behind_remote = self.is_behind_remote(path).await?;
        let has_conflicts = self.has_merge_conflicts(path).await?;

        Ok(RepoState {
            path: path.to_path_buf(),
            exists: true,
            has_uncommitted_changes,
            has_untracked_files,
            is_ahead_of_remote,
            is_behind_remote,
            has_conflicts,
            remote_url: actual_remote_url,
            current_branch,
        })
    }

    /// Clone a repository to the specified path
    pub async fn clone_repository(&self, repo: &Repository) -> Result<SyncResult> {
        let target_path = self.get_repo_directory(repo)?;
        let full_name = repo.full_name.as_deref().unwrap_or(&repo.name);

        info!(
            "Cloning repository: {} -> {}",
            full_name,
            target_path.display()
        );

        // Ensure parent directory exists
        if let Some(parent) = target_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directory")?;
        }

        // Choose clone URL based on availability
        let clone_url = if let Some(clone_url) = &repo.clone_url {
            clone_url.as_str()
        } else if let Some(ssh_url) = &repo.ssh_url {
            ssh_url
        } else {
            return Ok(SyncResult::Failed {
                path: target_path,
                error: "No valid clone URL found".to_string(),
            });
        };

        debug!("Using clone URL: {}", clone_url);

        // Perform the clone operation
        let output = AsyncCommand::new("git")
            .args(["clone", clone_url])
            .arg(&target_path)
            .output()
            .await
            .context("Failed to execute git clone")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(SyncResult::Failed {
                path: target_path,
                error: format!("Git clone failed: {}", stderr),
            });
        }

        // Set up timestamp preservation if configured
        if self.config.advanced.preserve_timestamps {
            if let Err(e) = self.preserve_git_timestamps(&target_path).await {
                warn!("Failed to preserve git timestamps: {}", e);
            }
        }

        // Verify clone integrity if configured
        if self.config.advanced.verify_clone {
            if let Err(e) = self.verify_repository_integrity(&target_path).await {
                warn!("Repository integrity verification failed: {}", e);
                if self.config.advanced.cleanup_on_error {
                    let _ = tokio::fs::remove_dir_all(&target_path).await;
                }
                return Ok(SyncResult::Failed {
                    path: target_path,
                    error: format!("Integrity verification failed: {}", e),
                });
            }
        }

        info!("Successfully cloned: {}", full_name);
        let branch = self.get_current_branch(&target_path).await.ok().flatten();
        Ok(SyncResult::Cloned {
            path: target_path,
            branch,
        })
    }

    /// Synchronize an existing repository
    pub async fn sync_repository(&self, repo: &Repository) -> Result<SyncResult> {
        let target_path = self.get_repo_directory(repo)?;

        if !target_path.exists() {
            return self.clone_repository(repo).await;
        }

        let full_name = repo.full_name.as_deref().unwrap_or(&repo.name);
        info!(
            "Syncing repository: {} at {}",
            full_name,
            target_path.display()
        );

        // Get clone URL for state analysis
        let clone_url = if let Some(clone_url) = &repo.clone_url {
            clone_url.as_str()
        } else if let Some(ssh_url) = &repo.ssh_url {
            ssh_url
        } else {
            return Ok(SyncResult::Skipped {
                path: target_path,
                reason: "No valid remote URL found".to_string(),
            });
        };

        // Analyze repository state
        let state = self.analyze_repo_state(&target_path, clone_url).await?;

        // Verify remote URL matches
        if let Some(actual_remote) = &state.remote_url {
            if !self.remote_urls_match(actual_remote, clone_url) {
                return Ok(SyncResult::Skipped {
                    path: target_path,
                    reason: format!(
                        "Remote URL mismatch: expected {}, found {}",
                        clone_url, actual_remote
                    ),
                });
            }
        }

        // Make sync decision based on strategy and repository state
        match self.config.sync.strategy.as_str() {
            "safe-pull" => self.safe_pull_sync(&state).await,
            "fetch-only" => self.fetch_only_sync(&state).await,
            "interactive" => self.interactive_sync(&state).await,
            _ => {
                warn!(
                    "Unknown sync strategy: {}, falling back to safe-pull",
                    self.config.sync.strategy
                );
                self.safe_pull_sync(&state).await
            }
        }
    }

    /// Safe pull strategy: only pull if no conflicts detected
    async fn safe_pull_sync(&self, state: &RepoState) -> Result<SyncResult> {
        let path = &state.path;

        // Check for conditions that prevent safe pulling
        if state.has_uncommitted_changes {
            if self.config.sync.auto_stash {
                info!("Auto-stashing uncommitted changes in {}", path.display());
                self.git_stash(path).await?;
            } else {
                return Ok(SyncResult::FetchedOnly {
                    path: path.clone(),
                    reason: "Repository has uncommitted changes".to_string(),
                });
            }
        }

        if state.has_conflicts {
            return Ok(SyncResult::FetchedOnly {
                path: path.clone(),
                reason: "Repository has unresolved conflicts".to_string(),
            });
        }

        if state.is_ahead_of_remote {
            return Ok(SyncResult::FetchedOnly {
                path: path.clone(),
                reason: "Repository is ahead of remote (has local commits)".to_string(),
            });
        }

        // If we're not behind remote, no update needed
        if !state.is_behind_remote {
            debug!("Repository is up to date: {}", path.display());
            return Ok(SyncResult::FetchedOnly {
                path: path.clone(),
                reason: "Repository is up to date".to_string(),
            });
        }

        // Perform the pull
        self.git_pull(path).await
    }

    /// Fetch-only strategy: never pull, only fetch
    async fn fetch_only_sync(&self, state: &RepoState) -> Result<SyncResult> {
        self.git_fetch(&state.path).await?;
        Ok(SyncResult::FetchedOnly {
            path: state.path.clone(),
            reason: "Fetch-only strategy (no pull performed)".to_string(),
        })
    }

    /// Interactive strategy: prompt user for conflicts
    async fn interactive_sync(&self, state: &RepoState) -> Result<SyncResult> {
        // For now, fall back to safe pull
        // In a real implementation, this would prompt the user
        warn!("Interactive mode not yet implemented, falling back to safe pull");
        self.safe_pull_sync(state).await
    }

    // Helper methods for git operations

    async fn has_uncommitted_changes(&self, path: &Path) -> Result<bool> {
        let output = AsyncCommand::new("git")
            .args(["status", "--porcelain"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to check git status")?;

        Ok(!output.stdout.is_empty())
    }

    async fn has_untracked_files(&self, path: &Path) -> Result<bool> {
        let output = AsyncCommand::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to check untracked files")?;

        Ok(!output.stdout.is_empty())
    }

    /// Get the current local branch name
    pub async fn get_current_branch(&self, path: &Path) -> Result<Option<String>> {
        let output = AsyncCommand::new("git")
            .args(["branch", "--show-current"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to get current branch")?;

        if output.status.success() && !output.stdout.is_empty() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(Some(branch))
        } else {
            Ok(None)
        }
    }

    async fn get_remote_url(&self, path: &Path) -> Result<Option<String>> {
        let output = AsyncCommand::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to get remote URL")?;

        if output.status.success() {
            let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(Some(url))
        } else {
            Ok(None)
        }
    }

    async fn git_fetch(&self, path: &Path) -> Result<()> {
        let output = AsyncCommand::new("git")
            .args(["fetch", "origin"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to fetch from remote")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Git fetch failed: {}", stderr));
        }

        Ok(())
    }

    async fn is_ahead_of_remote(&self, path: &Path) -> Result<bool> {
        let output = AsyncCommand::new("git")
            .args(["rev-list", "--count", "origin/HEAD..HEAD"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to check if ahead of remote")?;

        if output.status.success() {
            let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let count: u32 = count_str.parse().unwrap_or(0);
            Ok(count > 0)
        } else {
            Ok(false)
        }
    }

    async fn is_behind_remote(&self, path: &Path) -> Result<bool> {
        let output = AsyncCommand::new("git")
            .args(["rev-list", "--count", "HEAD..origin/HEAD"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to check if behind remote")?;

        if output.status.success() {
            let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let count: u32 = count_str.parse().unwrap_or(0);
            Ok(count > 0)
        } else {
            Ok(false)
        }
    }

    async fn has_merge_conflicts(&self, path: &Path) -> Result<bool> {
        let output = AsyncCommand::new("git")
            .args(["diff", "--name-only", "--diff-filter=U"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to check for merge conflicts")?;

        Ok(!output.stdout.is_empty())
    }

    async fn git_stash(&self, path: &Path) -> Result<()> {
        let output = AsyncCommand::new("git")
            .args(["stash", "push", "-m", "RepoSentry auto-stash"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to stash changes")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Git stash failed: {}", stderr));
        }

        Ok(())
    }

    async fn git_pull(&self, path: &Path) -> Result<SyncResult> {
        let mut args = vec!["pull", "origin"];

        if self.config.sync.fast_forward_only {
            args.push("--ff-only");
        }

        let output = AsyncCommand::new("git")
            .args(&args)
            .current_dir(path)
            .output()
            .await
            .context("Failed to pull from remote")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(SyncResult::Failed {
                path: path.to_path_buf(),
                error: format!("Git pull failed: {}", stderr),
            });
        }

        // Parse output to get number of commits updated
        let stdout = String::from_utf8_lossy(&output.stdout);
        let commits_updated = self.parse_pull_output(&stdout);

        // Update directory timestamp to match latest commit
        if self.config.advanced.preserve_timestamps {
            if let Err(e) = self.set_directory_commit_timestamp(path).await {
                warn!("Failed to set directory timestamp after pull: {}", e);
            }
        }

        let branch = self.get_current_branch(path).await.ok().flatten();
        info!(
            "Successfully pulled {} commits in {} (branch: {:?})",
            commits_updated,
            path.display(),
            branch
        );
        Ok(SyncResult::Pulled {
            path: path.to_path_buf(),
            commits_updated,
            branch,
        })
    }

    // Utility methods

    fn remote_urls_match(&self, actual: &str, expected: &str) -> bool {
        // Normalize URLs for comparison (handle https vs ssh)
        let normalize = |url: &str| -> String {
            url.replace("git@github.com:", "https://github.com/")
                .trim_end_matches(".git")
                .to_lowercase()
        };

        normalize(actual) == normalize(expected)
    }

    fn parse_pull_output(&self, output: &str) -> u32 {
        // Simple parsing of git pull output
        // Example: "Updating abc123..def456"
        if output.contains("Updating") {
            1 // At least one commit
        } else if output.contains("Already up to date") {
            0
        } else {
            1 // Default assumption
        }
    }

    // =========================================================================
    // Branch Operations (for most-recent strategy)
    // =========================================================================

    /// Fetch all remote branches with pruning
    pub async fn fetch_all_branches(&self, path: &Path) -> Result<()> {
        let output = AsyncCommand::new("git")
            .args(["fetch", "--all", "--prune"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to fetch all branches")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Git fetch --all failed: {}", stderr));
        }

        debug!("Fetched all branches for {}", path.display());
        Ok(())
    }

    /// Get the most recently updated remote branch
    ///
    /// Returns the branch name (without origin/ prefix) that has the most recent commit.
    /// Excludes branches matching the patterns in the branch config.
    pub async fn get_most_recent_branch(&self, path: &Path) -> Result<Option<String>> {
        // Get all remote branches sorted by commit date (most recent first)
        let output = AsyncCommand::new("git")
            .args([
                "for-each-ref",
                "--sort=-committerdate",
                "--format=%(refname:short)",
                "refs/remotes/origin/",
            ])
            .current_dir(path)
            .output()
            .await
            .context("Failed to get remote branches")?;

        if !output.status.success() {
            return Err(anyhow!("Git for-each-ref failed"));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            // Remove "origin/" prefix
            let branch = line.trim().strip_prefix("origin/").unwrap_or(line.trim());

            // Skip HEAD reference
            if branch == "HEAD" {
                continue;
            }

            // Check if branch is excluded by config patterns
            if self.config.branches.is_branch_excluded(branch) {
                debug!("Skipping excluded branch: {}", branch);
                continue;
            }

            return Ok(Some(branch.to_string()));
        }

        Ok(None)
    }

    /// Checkout a specific branch
    pub async fn checkout_branch(&self, path: &Path, branch: &str) -> Result<()> {
        // First, try to checkout if the branch already exists locally
        let output = AsyncCommand::new("git")
            .args(["checkout", branch])
            .current_dir(path)
            .output()
            .await
            .context("Failed to checkout branch")?;

        if output.status.success() {
            info!("Checked out branch {} in {}", branch, path.display());
            return Ok(());
        }

        // Branch doesn't exist locally, create a tracking branch
        let output = AsyncCommand::new("git")
            .args(["checkout", "-b", branch, &format!("origin/{}", branch)])
            .current_dir(path)
            .output()
            .await
            .context("Failed to create tracking branch")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Git checkout failed: {}", stderr));
        }

        info!(
            "Created and checked out tracking branch {} in {}",
            branch,
            path.display()
        );
        Ok(())
    }

    /// Check if the repository has any local changes (uncommitted or untracked)
    ///
    /// This is a convenience method that combines multiple checks.
    /// Returns true if there are ANY local changes that would prevent a safe branch switch.
    pub async fn has_any_local_changes(&self, path: &Path) -> Result<bool> {
        // Check for uncommitted changes (staged or unstaged)
        let status_output = AsyncCommand::new("git")
            .args(["status", "--porcelain"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to check git status")?;

        if !status_output.status.success() {
            return Err(anyhow!("Git status failed"));
        }

        let status = String::from_utf8_lossy(&status_output.stdout);
        if !status.trim().is_empty() {
            debug!(
                "Repository {} has local changes:\n{}",
                path.display(),
                status.trim()
            );
            return Ok(true);
        }

        Ok(false)
    }

    /// Get the number of commits the local branch is ahead of the remote tracking branch
    pub async fn commits_ahead_of_remote(&self, path: &Path, branch: &str) -> Result<u32> {
        let output = AsyncCommand::new("git")
            .args([
                "rev-list",
                "--count",
                &format!("origin/{}..{}", branch, branch),
            ])
            .current_dir(path)
            .output()
            .await
            .context("Failed to count commits ahead")?;

        if output.status.success() {
            let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(count_str.parse().unwrap_or(0))
        } else {
            Ok(0)
        }
    }

    /// Set the directory's modification timestamp to match the latest git commit
    ///
    /// This mirrors the behavior of the legacy bash script which preserves
    /// the timestamp so file managers can sort repositories by activity.
    pub async fn set_directory_commit_timestamp(&self, path: &Path) -> Result<()> {
        // Get the latest commit timestamp (Unix epoch seconds)
        let output = AsyncCommand::new("git")
            .args(["log", "-1", "--format=%ct"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to get latest commit timestamp")?;

        if !output.status.success() {
            return Err(anyhow!("Git log command failed"));
        }

        let timestamp_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if timestamp_str.is_empty() {
            return Err(anyhow!("No commits found in repository"));
        }

        let timestamp: i64 = timestamp_str
            .parse()
            .context("Invalid timestamp from git log")?;

        // Validate timestamp (between 2005 when Git was created and 2050)
        const MIN_TIMESTAMP: i64 = 1104537600; // 2005-01-01
        const MAX_TIMESTAMP: i64 = 2524608000; // 2050-01-01

        if !(MIN_TIMESTAMP..=MAX_TIMESTAMP).contains(&timestamp) {
            return Err(anyhow!(
                "Commit timestamp {} is outside valid range",
                timestamp
            ));
        }

        // Set the directory's modification time
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let mtime = UNIX_EPOCH + Duration::from_secs(timestamp as u64);
        let atime = SystemTime::now(); // Keep access time as now

        filetime::set_file_times(path, atime.into(), mtime.into())
            .context("Failed to set directory timestamp")?;

        debug!(
            "Set directory timestamp to {} for {}",
            timestamp,
            path.display()
        );
        Ok(())
    }

    async fn preserve_git_timestamps(&self, path: &Path) -> Result<()> {
        // Set the directory timestamp to match the latest commit
        self.set_directory_commit_timestamp(path).await
    }

    async fn verify_repository_integrity(&self, path: &Path) -> Result<()> {
        let output = AsyncCommand::new("git")
            .args(["fsck"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to verify repository integrity")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Repository integrity check failed: {}", stderr));
        }

        Ok(())
    }

    // =========================================================================
    // RepoSpec-based methods (provider-agnostic)
    // =========================================================================

    /// Sync a repository using RepoSpec (provider-agnostic)
    ///
    /// This method uses the pre-computed local_path and clone_url from RepoSpec,
    /// making it work with any git hosting provider.
    ///
    /// When the "most-recent" branch strategy is enabled, this will:
    /// 1. Check for local changes (skip if any)
    /// 2. Fetch all remote branches
    /// 3. Determine the most recently updated branch
    /// 4. Switch to that branch if different from current
    /// 5. Pull the latest changes
    pub async fn sync_from_spec(&self, spec: &crate::discovery::RepoSpec) -> Result<SyncResult> {
        let target_path = &spec.local_path;

        if !target_path.exists() {
            return self.clone_from_spec(spec).await;
        }

        info!(
            "Syncing repository: {} at {}",
            spec.full_name(),
            target_path.display()
        );

        // CRITICAL: Check for local changes FIRST - if any exist, skip entirely
        // This is the "Dropbox for Git" safety rule: never lose user data
        if self.has_any_local_changes(target_path).await? {
            return Ok(SyncResult::Skipped {
                path: target_path.clone(),
                reason: "Repository has local changes (uncommitted or untracked files)".to_string(),
            });
        }

        // Analyze current state for conflicts
        let state = self
            .analyze_repo_state(target_path, &spec.clone_url)
            .await
            .context("Failed to analyze repository state")?;

        if state.has_conflicts {
            warn!(
                "Repository has conflicts, falling back to fetch-only: {}",
                spec.full_name()
            );
            self.git_fetch(target_path).await?;
            return Ok(SyncResult::FetchedOnly {
                path: target_path.clone(),
                reason: "Repository has unresolved conflicts".to_string(),
            });
        }

        // Handle fetch-only strategy
        if self.config.sync.strategy == "fetch-only" {
            self.git_fetch(target_path).await?;
            return Ok(SyncResult::FetchedOnly {
                path: target_path.clone(),
                reason: "Fetch-only strategy configured".to_string(),
            });
        }

        // Check if we should use the most-recent branch strategy
        if self.config.branches.is_most_recent_strategy() {
            return self.sync_with_most_recent_branch(target_path).await;
        }

        // Default: just pull the current branch
        self.git_pull(target_path).await
    }

    /// Sync using the "most-recent" branch strategy
    ///
    /// This fetches all branches, finds the one with the most recent commit,
    /// switches to it if necessary, and pulls.
    async fn sync_with_most_recent_branch(&self, path: &Path) -> Result<SyncResult> {
        // Fetch all branches to get latest refs
        self.fetch_all_branches(path).await?;

        // Get current branch
        let current_branch = self
            .get_current_branch(path)
            .await?
            .unwrap_or_else(|| "unknown".to_string());

        // Find the most recently updated branch
        let most_recent_branch = self.get_most_recent_branch(path).await?;

        let target_branch = match most_recent_branch {
            Some(branch) => branch,
            None => {
                // No branches found, just pull current
                return self.git_pull(path).await;
            }
        };

        // Check if we need to switch branches
        if current_branch != target_branch {
            info!(
                "Switching from '{}' to '{}' (most recent activity) in {}",
                current_branch,
                target_branch,
                path.display()
            );

            // Switch to the target branch
            self.checkout_branch(path, &target_branch).await?;

            // Pull the new branch
            let pull_result = self.git_pull(path).await?;

            // Return BranchSwitched result with pull info
            let commits_updated = match &pull_result {
                SyncResult::Pulled {
                    commits_updated, ..
                } => *commits_updated,
                _ => 0,
            };

            return Ok(SyncResult::BranchSwitched {
                path: path.to_path_buf(),
                from_branch: current_branch,
                to_branch: target_branch,
                commits_updated,
            });
        }

        // No branch switch needed, just pull
        self.git_pull(path).await
    }

    /// Clone a repository using RepoSpec (provider-agnostic)
    pub async fn clone_from_spec(&self, spec: &crate::discovery::RepoSpec) -> Result<SyncResult> {
        let target_path = &spec.local_path;

        info!(
            "Cloning repository: {} to {}",
            spec.full_name(),
            target_path.display()
        );

        // Create parent directory if it doesn't exist
        if let Some(parent) = target_path.parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .context("Failed to create parent directory")?;
            }
        }

        // Clone the repository
        let output = AsyncCommand::new("git")
            .args(["clone", &spec.clone_url, &target_path.to_string_lossy()])
            .output()
            .await
            .context("Failed to clone repository")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);

            // Cleanup on error if configured
            if self.config.advanced.cleanup_on_error && target_path.exists() {
                let _ = tokio::fs::remove_dir_all(target_path).await;
            }

            return Err(anyhow!("Clone failed: {}", stderr));
        }

        // Verify clone if configured
        if self.config.advanced.verify_clone {
            if let Err(e) = self.verify_repository_integrity(target_path).await {
                if self.config.advanced.cleanup_on_error {
                    let _ = tokio::fs::remove_dir_all(target_path).await;
                }
                return Err(e);
            }
        }

        // Preserve timestamps if configured
        if self.config.advanced.preserve_timestamps {
            if let Err(e) = self.preserve_git_timestamps(target_path).await {
                warn!("Failed to preserve timestamps: {}", e);
            }
        }

        // If most-recent strategy is enabled, switch to the most active branch after clone
        let branch = if self.config.branches.is_most_recent_strategy() {
            self.fetch_all_branches(target_path).await?;
            if let Some(most_recent) = self.get_most_recent_branch(target_path).await? {
                let current = self.get_current_branch(target_path).await?.unwrap_or_default();
                if current != most_recent {
                    info!(
                        "Switching from {} to {} (most recent branch) after clone",
                        current, most_recent
                    );
                    self.checkout_branch(target_path, &most_recent).await?;
                }
                Some(most_recent)
            } else {
                self.get_current_branch(target_path).await.ok().flatten()
            }
        } else {
            self.get_current_branch(target_path).await.ok().flatten()
        };

        Ok(SyncResult::Cloned {
            path: target_path.clone(),
            branch,
        })
    }

    /// Analyze repository state using RepoSpec
    pub async fn analyze_from_spec(&self, spec: &crate::discovery::RepoSpec) -> Result<RepoState> {
        self.analyze_repo_state(&spec.local_path, &spec.clone_url)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remote_url_matching() {
        let config = Config::default();
        let git_client = GitClient::new(config);

        // Test HTTPS vs SSH URL matching
        assert!(git_client.remote_urls_match(
            "git@github.com:user/repo.git",
            "https://github.com/user/repo"
        ));

        assert!(git_client.remote_urls_match(
            "https://github.com/user/repo.git",
            "https://github.com/user/repo"
        ));

        assert!(!git_client.remote_urls_match(
            "https://github.com/user/repo1",
            "https://github.com/user/repo2"
        ));
    }

    #[test]
    fn test_directory_path_construction() {
        // Test the core path construction logic used by get_repo_directory
        let base = PathBuf::from("/tmp/repos");

        // Test organization separation enabled
        let parts: Vec<&str> = "octocat/Hello-World".split('/').collect();
        let org_enabled_path = if parts.len() > 1 {
            base.join(&parts[0]).join(&parts[1])
        } else {
            base.join("Hello-World")
        };
        assert_eq!(
            org_enabled_path,
            PathBuf::from("/tmp/repos/octocat/Hello-World")
        );

        // Test organization separation disabled with prefix
        let prefix_path = base.join("octocat-Hello-World");
        assert_eq!(prefix_path, PathBuf::from("/tmp/repos/octocat-Hello-World"));

        // Test simple repo name fallback
        let simple_path = base.join("Hello-World");
        assert_eq!(simple_path, PathBuf::from("/tmp/repos/Hello-World"));
    }

    #[test]
    fn test_url_normalization() {
        let config = Config::default();
        let _git_client = GitClient::new(config);

        // Test URL normalization logic
        let ssh_url = "git@github.com:user/repo.git";
        let https_url1 = "https://github.com/user/repo.git";
        let https_url2 = "https://github.com/user/repo";

        // Extract the core path from different URL formats
        let extract_path = |url: &str| -> String {
            if url.starts_with("git@") {
                url.split(':').nth(1).unwrap_or("").replace(".git", "")
            } else if url.starts_with("https://github.com/") {
                url.replacen("https://github.com/", "", 1)
                    .replace(".git", "")
            } else {
                url.to_string()
            }
        };

        let ssh_path = extract_path(ssh_url);
        let https_path1 = extract_path(https_url1);
        let https_path2 = extract_path(https_url2);

        assert_eq!(ssh_path, "user/repo");
        assert_eq!(https_path1, "user/repo");
        assert_eq!(https_path2, "user/repo");
        assert_eq!(ssh_path, https_path1);
        assert_eq!(https_path1, https_path2);
    }
}
