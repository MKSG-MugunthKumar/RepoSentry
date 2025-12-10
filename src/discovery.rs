//! Repository discovery abstraction layer
//!
//! This module provides a provider-agnostic interface for discovering repositories
//! from various sources (GitHub, GitLab, Codeberg, local directories, etc.)

use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;

/// Clone method preference for a repository
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CloneMethod {
    /// Use SSH (git@github.com:user/repo.git)
    #[default]
    Ssh,
    /// Use HTTPS (https://github.com/user/repo.git)
    Https,
}

/// Provider-agnostic repository specification
///
/// This struct contains all information needed to clone and manage a repository,
/// regardless of where it was discovered from.
#[derive(Debug, Clone)]
pub struct RepoSpec {
    /// Repository name (e.g., "reposentry")
    pub name: String,

    /// Owner/organization name (e.g., "MKSG-MugunthKumar")
    pub owner: String,

    /// Full clone URL (SSH or HTTPS depending on clone_method)
    pub clone_url: String,

    /// Alternative clone URL (the other protocol)
    pub clone_url_alt: Option<String>,

    /// Preferred clone method
    pub clone_method: CloneMethod,

    /// Local path where this repo should be cloned
    /// Computed based on config (base_dir, separate_org_dirs, etc.)
    pub local_path: PathBuf,

    /// Whether the repository is a fork
    pub is_fork: bool,

    /// Whether the repository is archived
    pub is_archived: bool,

    /// Repository size in bytes (if known)
    pub size_bytes: Option<u64>,

    /// Default branch name
    pub default_branch: Option<String>,

    /// Source provider (for logging/display)
    pub provider: String,
}

impl RepoSpec {
    /// Check if the repository already exists locally
    pub fn exists_locally(&self) -> bool {
        self.local_path.exists() && self.local_path.join(".git").exists()
    }

    /// Get display name (owner/name format)
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// Trait for repository discovery from various providers
///
/// Implement this trait to add support for new git hosting providers
/// like GitLab, Codeberg, Gitea, or even local directory scanning.
#[async_trait]
pub trait Discovery: Send + Sync {
    /// Discover repositories from this provider
    ///
    /// Returns a list of RepoSpec that can be passed to SyncEngine
    async fn discover(&self) -> Result<Vec<RepoSpec>>;

    /// Provider name for display/logging
    fn provider_name(&self) -> &'static str;

    /// Check if this provider is properly configured/authenticated
    async fn is_available(&self) -> bool;
}

/// Aggregates multiple discovery sources
pub struct MultiDiscovery {
    sources: Vec<Box<dyn Discovery>>,
}

impl MultiDiscovery {
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    pub fn add_source(&mut self, source: Box<dyn Discovery>) {
        self.sources.push(source);
    }

    /// Discover from all configured sources
    pub async fn discover_all(&self) -> Result<Vec<RepoSpec>> {
        let mut all_repos = Vec::new();

        for source in &self.sources {
            if source.is_available().await {
                match source.discover().await {
                    Ok(repos) => {
                        tracing::info!(
                            "Discovered {} repositories from {}",
                            repos.len(),
                            source.provider_name()
                        );
                        all_repos.extend(repos);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to discover from {}: {}", source.provider_name(), e);
                    }
                }
            }
        }

        Ok(all_repos)
    }
}

impl Default for MultiDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// GitHub Discovery Implementation
// =============================================================================

use crate::{Config, GitHubClient};
use std::sync::Arc;

/// GitHub repository discovery implementation
pub struct GitHubDiscovery {
    client: GitHubClient,
    config: Arc<Config>,
}

impl GitHubDiscovery {
    /// Create a new GitHub discovery instance
    pub async fn new(config: Config) -> Result<Self> {
        let client = GitHubClient::new(&config).await?;
        Ok(Self {
            client,
            config: Arc::new(config),
        })
    }

    /// Create from existing client (for reuse)
    pub fn with_client(client: GitHubClient, config: Config) -> Self {
        Self {
            client,
            config: Arc::new(config),
        }
    }

    /// Convert octocrab Repository to our RepoSpec
    fn repo_to_spec(&self, repo: &octocrab::models::Repository) -> RepoSpec {
        let owner = repo
            .owner
            .as_ref()
            .map(|o| o.login.clone())
            .unwrap_or_else(|| "unknown".to_string());

        // Compute local path based on config
        let base_dir = shellexpand::full(&self.config.base_directory)
            .unwrap_or_else(|_| std::borrow::Cow::Borrowed(&self.config.base_directory));

        let local_path = if self.config.organization.separate_org_dirs {
            PathBuf::from(base_dir.as_ref())
                .join(&owner)
                .join(&repo.name)
        } else {
            PathBuf::from(base_dir.as_ref()).join(&repo.name)
        };

        // Prefer SSH URL, fall back to clone_url (HTTPS)
        let ssh_url = repo.ssh_url.clone();
        let https_url = repo.clone_url.as_ref().map(|u| u.to_string());

        let (clone_url, clone_url_alt, clone_method) = match (&ssh_url, &https_url) {
            (Some(ssh), https) => (ssh.clone(), https.clone(), CloneMethod::Ssh),
            (None, Some(https)) => (https.clone(), None, CloneMethod::Https),
            (None, None) => (
                format!("git@github.com:{}/{}.git", owner, repo.name),
                None,
                CloneMethod::Ssh,
            ),
        };

        RepoSpec {
            name: repo.name.clone(),
            owner,
            clone_url,
            clone_url_alt,
            clone_method,
            local_path,
            is_fork: repo.fork.unwrap_or(false),
            is_archived: repo.archived.unwrap_or(false),
            size_bytes: repo.size.map(|kb| kb as u64 * 1024),
            default_branch: repo.default_branch.clone(),
            provider: "github".to_string(),
        }
    }

    /// Get the authenticated username
    pub fn username(&self) -> &str {
        self.client.username()
    }
}

#[async_trait]
impl Discovery for GitHubDiscovery {
    async fn discover(&self) -> Result<Vec<RepoSpec>> {
        let repositories = self.client.get_all_repositories(&self.config).await?;

        let specs: Vec<RepoSpec> = repositories
            .iter()
            .map(|repo| self.repo_to_spec(repo))
            .collect();

        Ok(specs)
    }

    fn provider_name(&self) -> &'static str {
        "GitHub"
    }

    async fn is_available(&self) -> bool {
        // If we have a client, we're authenticated
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_spec_full_name() {
        let spec = RepoSpec {
            name: "reposentry".to_string(),
            owner: "MKSG".to_string(),
            clone_url: "git@github.com:MKSG/reposentry.git".to_string(),
            clone_url_alt: Some("https://github.com/MKSG/reposentry.git".to_string()),
            clone_method: CloneMethod::Ssh,
            local_path: PathBuf::from("/home/user/dev/MKSG/reposentry"),
            is_fork: false,
            is_archived: false,
            size_bytes: Some(1024 * 1024),
            default_branch: Some("main".to_string()),
            provider: "github".to_string(),
        };

        assert_eq!(spec.full_name(), "MKSG/reposentry");
    }

    #[test]
    fn test_repo_spec_exists_locally() {
        let spec = RepoSpec {
            name: "nonexistent".to_string(),
            owner: "test".to_string(),
            clone_url: "git@github.com:test/nonexistent.git".to_string(),
            clone_url_alt: None,
            clone_method: CloneMethod::Ssh,
            local_path: PathBuf::from("/nonexistent/path/repo"),
            is_fork: false,
            is_archived: false,
            size_bytes: None,
            default_branch: None,
            provider: "test".to_string(),
        };

        assert!(!spec.exists_locally());
    }

    #[test]
    fn test_clone_method_default() {
        assert_eq!(CloneMethod::default(), CloneMethod::Ssh);
    }
}
