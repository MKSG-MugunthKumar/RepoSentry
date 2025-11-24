use anyhow::{anyhow, Context, Result};
use octocrab::models::Repository;
use octocrab::Octocrab;
use std::env;
use std::process::Command;
use tracing::{debug, info, warn};

use crate::config::Config;

/// GitHub client wrapper with authentication management
pub struct GitHubClient {
    client: Octocrab,
    username: String,
}

/// GitHub authentication strategies
#[derive(Debug, Clone)]
pub enum AuthStrategy {
    /// Use GitHub CLI authentication
    GitHubCLI,
    /// Use environment variable token
    EnvironmentToken,
}

impl GitHubClient {
    /// Create a new GitHub client with automatic authentication
    pub async fn new(config: &Config) -> Result<Self> {
        let (auth_strategy, token) = Self::detect_authentication(config)?;

        info!("Using authentication strategy: {:?}", auth_strategy);

        let client = Octocrab::builder()
            .personal_token(token)
            .build()
            .context("Failed to create GitHub client")?;

        // Get authenticated user information
        let user = client
            .current()
            .user()
            .await
            .context("Failed to get current user information. Check your authentication.")?;

        let username = config
            .github
            .username
            .clone()
            .unwrap_or_else(|| user.login.clone());

        info!("Authenticated as GitHub user: {}", username);

        Ok(Self { client, username })
    }

    /// Detect and obtain GitHub authentication
    fn detect_authentication(config: &Config) -> Result<(AuthStrategy, String)> {
        match config.github.auth_method.as_str() {
            "auto" => {
                // Try GitHub CLI first, then environment token
                if let Ok(token) = Self::try_github_cli() {
                    Ok((AuthStrategy::GitHubCLI, token))
                } else if let Ok(token) = Self::try_environment_token() {
                    Ok((AuthStrategy::EnvironmentToken, token))
                } else {
                    Err(anyhow!(
                        "No GitHub authentication found. Please either:\n\
                         1. Install and authenticate GitHub CLI: gh auth login\n\
                         2. Set GITHUB_TOKEN environment variable\n\
                         3. Run: reposentry auth setup"
                    ))
                }
            }
            "gh_cli" => {
                let token = Self::try_github_cli()
                    .context("GitHub CLI authentication failed. Run: gh auth login")?;
                Ok((AuthStrategy::GitHubCLI, token))
            }
            "token" => {
                let token = Self::try_environment_token()
                    .context("GITHUB_TOKEN environment variable not found or invalid")?;
                Ok((AuthStrategy::EnvironmentToken, token))
            }
            other => Err(anyhow!("Unknown auth method: {}", other)),
        }
    }

    /// Try to get token from GitHub CLI
    fn try_github_cli() -> Result<String> {
        debug!("Attempting GitHub CLI authentication");

        // Check if gh CLI is installed
        if !Self::is_command_available("gh") {
            return Err(anyhow!("GitHub CLI (gh) is not installed"));
        }

        // Check if user is authenticated
        let auth_status = Command::new("gh")
            .args(&["auth", "status"])
            .output()
            .context("Failed to check GitHub CLI auth status")?;

        if !auth_status.status.success() {
            return Err(anyhow!(
                "GitHub CLI is not authenticated. Run: gh auth login"
            ));
        }

        // Get the token
        let token_output = Command::new("gh")
            .args(&["auth", "token"])
            .output()
            .context("Failed to get GitHub CLI token")?;

        if !token_output.status.success() {
            return Err(anyhow!(
                "Failed to retrieve token from GitHub CLI: {}",
                String::from_utf8_lossy(&token_output.stderr)
            ));
        }

        let token = String::from_utf8(token_output.stdout)
            .context("GitHub CLI token is not valid UTF-8")?
            .trim()
            .to_string();

        if token.is_empty() {
            return Err(anyhow!("GitHub CLI returned empty token"));
        }

        debug!("Successfully obtained token from GitHub CLI");
        Ok(token)
    }

    /// Try to get token from environment variable
    fn try_environment_token() -> Result<String> {
        debug!("Attempting environment variable authentication");

        let token = env::var("GITHUB_TOKEN")
            .context("GITHUB_TOKEN environment variable not set")?;

        if token.is_empty() {
            return Err(anyhow!("GITHUB_TOKEN is empty"));
        }

        if !token.starts_with("ghp_") && !token.starts_with("gho_") && !token.starts_with("ghs_") {
            warn!("GITHUB_TOKEN doesn't look like a valid GitHub token (should start with ghp_, gho_, or ghs_)");
        }

        debug!("Successfully found GITHUB_TOKEN environment variable");
        Ok(token)
    }

    /// Check if a command is available in PATH
    fn is_command_available(command: &str) -> bool {
        Command::new("which")
            .arg(command)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// Get the authenticated username
    pub fn username(&self) -> &str {
        &self.username
    }

    /// List all repositories for the authenticated user
    pub async fn list_user_repositories(&self) -> Result<Vec<Repository>> {
        debug!("Fetching user repositories for: {}", self.username);

        let mut repositories = Vec::new();
        let mut page = 1u8;

        loop {
            let page_repos = self
                .client
                .current()
                .list_repos_for_authenticated_user()
                .per_page(100)
                .page(page)
                .send()
                .await
                .with_context(|| format!("Failed to fetch repositories page {}", page))?;

            let items = page_repos.items;
            if items.is_empty() {
                break;
            }

            repositories.extend(items);

            // GitHub API pagination limit for u8
            if page >= 255 {
                warn!("Reached maximum pagination limit (255 pages)");
                break;
            }
            page += 1;
        }

        info!("Found {} user repositories", repositories.len());
        Ok(repositories)
    }

    /// List all organizations the user is a member of
    pub async fn list_user_organizations(&self) -> Result<Vec<String>> {
        debug!("Fetching organizations for user: {}", self.username);

        let orgs = self
            .client
            .current()
            .list_org_memberships_for_authenticated_user()
            .per_page(100)
            .send()
            .await
            .context("Failed to fetch user organizations")?;

        let org_names: Vec<String> = orgs.items.into_iter().map(|org| org.organization.login).collect();

        info!("Found {} organizations: {:?}", org_names.len(), org_names);
        Ok(org_names)
    }

    /// List repositories for a specific organization
    pub async fn list_organization_repositories(&self, org: &str) -> Result<Vec<Repository>> {
        debug!("Fetching repositories for organization: {}", org);

        let mut repositories = Vec::new();
        let mut page = 1u8;

        loop {
            let page_repos = self
                .client
                .orgs(org)
                .list_repos()
                .per_page(100)
                .page(page)
                .send()
                .await
                .with_context(|| {
                    format!("Failed to fetch repositories for organization {} page {}", org, page)
                })?;

            let items = page_repos.items;
            if items.is_empty() {
                break;
            }

            repositories.extend(items);

            // GitHub API pagination limit for u8
            if page >= 255 {
                warn!("Reached maximum pagination limit (255 pages) for org: {}", org);
                break;
            }
            page += 1;
        }

        info!(
            "Found {} repositories for organization: {}",
            repositories.len(),
            org
        );
        Ok(repositories)
    }

    /// Get all repositories (user + organizations) with filtering applied
    pub async fn get_all_repositories(&self, config: &Config) -> Result<Vec<Repository>> {
        let mut all_repositories = Vec::new();

        // Get user repositories
        let user_repos = self.list_user_repositories().await?;
        all_repositories.extend(user_repos);

        // Get organization repositories if enabled
        if config.github.include_organizations {
            let organizations = self.list_user_organizations().await?;

            for org in organizations {
                match self.list_organization_repositories(&org).await {
                    Ok(org_repos) => {
                        all_repositories.extend(org_repos);
                    }
                    Err(e) => {
                        warn!("Failed to fetch repositories for organization {}: {}", org, e);
                        continue;
                    }
                }
            }
        }

        info!(
            "Total repositories before filtering: {}",
            all_repositories.len()
        );

        // Apply filters
        let filtered_repositories = self.apply_filters(all_repositories, config).await?;

        info!(
            "Repositories after filtering: {}",
            filtered_repositories.len()
        );

        Ok(filtered_repositories)
    }

    /// Apply configuration filters to repositories
    async fn apply_filters(
        &self,
        repositories: Vec<Repository>,
        config: &Config,
    ) -> Result<Vec<Repository>> {
        let mut filtered = Vec::new();

        for repo in repositories {
            // Skip if matches exclusion patterns
            if self.matches_exclusion_pattern(&repo.name, &config.github.exclude_patterns) {
                debug!("Excluding repository due to pattern match: {}", repo.name);
                continue;
            }

            // Skip forks if not included
            if repo.fork == Some(true) && !config.github.include_forks {
                debug!("Excluding fork repository: {}", repo.name);
                continue;
            }

            // Check age filter
            if let Some(updated_at) = repo.updated_at {
                if config.should_filter_by_age(&updated_at) {
                    debug!("Excluding repository due to age: {}", repo.name);
                    continue;
                }
            }

            // Check size filter
            if let Some(size_kb) = repo.size {
                let size_bytes = size_kb * 1024; // GitHub API returns size in KB
                if config.should_filter_by_size(size_bytes as u64) {
                    debug!(
                        "Excluding repository due to size ({} KB): {}",
                        size_kb, repo.name
                    );
                    continue;
                }
            }

            filtered.push(repo);
        }

        Ok(filtered)
    }

    /// Check if repository name matches any exclusion pattern
    fn matches_exclusion_pattern(&self, name: &str, patterns: &[String]) -> bool {
        patterns.iter().any(|pattern| {
            // Simple glob pattern matching
            if pattern.contains('*') {
                let pattern_regex = pattern
                    .replace('.', r"\.")
                    .replace('*', ".*");

                regex::Regex::new(&format!("^{}$", pattern_regex))
                    .map(|re| re.is_match(name))
                    .unwrap_or(false)
            } else {
                name == pattern
            }
        })
    }
}

/// Utility functions for GitHub authentication setup
pub mod auth_setup {
    use super::*;

    /// Interactive authentication setup guide
    pub async fn setup_authentication() -> Result<()> {
        println!("🔧 RepoSentry Authentication Setup");
        println!();

        // Check if gh CLI is available
        if Command::new("which").arg("gh").output()?.status.success() {
            println!("✅ GitHub CLI (gh) is installed");

            // Check if already authenticated
            if Command::new("gh").args(&["auth", "status"]).output()?.status.success() {
                println!("✅ GitHub CLI is already authenticated");
                return Ok(());
            } else {
                println!("🔄 GitHub CLI needs authentication");
                println!("Run: gh auth login");
                return Ok(());
            }
        }

        // Suggest GitHub CLI installation
        println!("❌ GitHub CLI (gh) is not installed");
        println!();
        println!("Recommended setup:");
        println!("1. Install GitHub CLI:");

        #[cfg(target_os = "macos")]
        println!("   brew install gh");

        #[cfg(target_os = "linux")]
        println!("   See: https://github.com/cli/cli/blob/trunk/docs/install_linux.md");

        #[cfg(target_os = "windows")]
        println!("   winget install --id GitHub.cli");

        println!();
        println!("2. Authenticate:");
        println!("   gh auth login");
        println!();
        println!("Alternative: Set GITHUB_TOKEN environment variable");
        println!("   export GITHUB_TOKEN=your_token_here");

        Ok(())
    }

    /// Test current authentication
    pub async fn test_authentication(config: &Config) -> Result<()> {
        println!("🔍 Testing GitHub authentication...");

        match GitHubClient::new(config).await {
            Ok(client) => {
                println!("✅ Authentication successful");
                println!("   Username: {}", client.username());

                // Test basic API access
                match client.list_user_organizations().await {
                    Ok(orgs) => {
                        if orgs.is_empty() {
                            println!("   Organizations: None");
                        } else {
                            println!("   Organizations: {}", orgs.join(", "));
                        }
                    }
                    Err(e) => {
                        println!("⚠️  Could not list organizations: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("❌ Authentication failed: {}", e);
                println!();
                println!("To fix this, run: reposentry auth setup");
            }
        }

        Ok(())
    }
}