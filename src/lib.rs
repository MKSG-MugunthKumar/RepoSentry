//! RepoSentry - Intelligent Git Repository Synchronization Daemon
//!
//! RepoSentry automatically keeps local repository collections in sync with remote origins
//! while intelligently preventing data loss from careless automated pulls.
//!
//! ## Core Features
//!
//! - **GitHub Integration**: Automatic repository discovery via GitHub API
//! - **Intelligent Filtering**: Age and size-based repository filtering
//! - **Configuration Management**: YAML-based configuration with XDG compliance
//! - **Authentication**: GitHub CLI and token-based authentication support
//! - **Organization Support**: Automatic organization repository discovery
//!
//! ## Modules
//!
//! - [`config`]: Configuration management and parsing
//! - [`github`]: GitHub API integration and authentication

pub mod config;
pub mod github;
pub mod git;
pub mod sync;

pub use config::Config;
pub use github::GitHubClient;
pub use git::{GitClient, SyncResult, RepoState};
pub use sync::{SyncEngine, SyncSummary};