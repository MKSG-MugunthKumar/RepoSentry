use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use shellexpand;
use std::path::{Path, PathBuf};

/// Main configuration structure for RepoSentry
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    /// Base directory for repository synchronization
    pub base_directory: String,

    /// Repository filtering configuration
    #[serde(default)]
    pub filters: FilterConfig,

    /// GitHub authentication and discovery settings
    #[serde(default)]
    pub github: GitHubConfig,

    /// Synchronization behavior settings
    #[serde(default)]
    pub sync: SyncConfig,

    /// Daemon configuration
    #[serde(default)]
    pub daemon: DaemonConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Directory structure organization
    #[serde(default)]
    pub organization: OrganizationConfig,

    /// Advanced settings
    #[serde(default)]
    pub advanced: AdvancedConfig,
}

/// Repository filtering configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FilterConfig {
    /// Age-based filtering
    #[serde(default)]
    pub age: AgeFilter,

    /// Size-based filtering
    #[serde(default)]
    pub size: SizeFilter,
}

/// Age-based repository filtering
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AgeFilter {
    /// Maximum age for repositories to be cloned
    pub max_age: Option<String>, // "1month", "3month", "6month"
}

/// Size-based repository filtering
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct SizeFilter {
    /// Maximum size for repositories to be cloned
    pub max_size: Option<String>, // "100MB", "1GB"
}

/// GitHub configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GitHubConfig {
    /// Authentication method
    #[serde(default = "default_auth_method")]
    pub auth_method: String, // "auto", "gh_cli", "token"

    /// GitHub username (auto-detected if null)
    pub username: Option<String>,

    /// Include organization repositories
    #[serde(default = "default_true")]
    pub include_organizations: bool,

    /// Repository exclusion patterns
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    /// Include forked repositories
    #[serde(default)]
    pub include_forks: bool,
}

/// Synchronization configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SyncConfig {
    /// Sync strategy
    #[serde(default = "default_sync_strategy")]
    pub strategy: String, // "safe-pull", "fetch-only", "interactive"

    /// Maximum parallel operations
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,

    /// Timeout for git operations in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Auto-stash uncommitted changes
    #[serde(default)]
    pub auto_stash: bool,

    /// Fast-forward only pulls
    #[serde(default = "default_true")]
    pub fast_forward_only: bool,
}

/// Daemon configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DaemonConfig {
    /// Enable daemon mode
    #[serde(default)]
    pub enabled: bool,

    /// Sync interval
    #[serde(default = "default_interval")]
    pub interval: String, // "30m"

    /// PID file location
    #[serde(default = "default_pid_file")]
    pub pid_file: String,

    /// Log file location
    #[serde(default = "default_log_file")]
    pub log_file: String,
}

/// Logging configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String, // "info"

    /// Log format
    #[serde(default = "default_log_format")]
    pub format: String, // "compact"

    /// Enable colored output
    #[serde(default = "default_true")]
    pub color: bool,
}

/// Organization directory configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OrganizationConfig {
    /// Create separate directories for organizations
    #[serde(default = "default_true")]
    pub separate_org_dirs: bool,

    /// Handle repository name conflicts
    #[serde(default = "default_conflict_resolution")]
    pub conflict_resolution: String, // "prefix-org"
}

/// Advanced configuration options
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AdvancedConfig {
    /// Preserve git timestamps
    #[serde(default = "default_true")]
    pub preserve_timestamps: bool,

    /// Verify repository integrity after clone
    #[serde(default = "default_true")]
    pub verify_clone: bool,

    /// Clean up failed clone attempts
    #[serde(default = "default_true")]
    pub cleanup_on_error: bool,

    /// Repository metadata cache duration
    #[serde(default = "default_cache_duration")]
    pub cache_duration: String, // "1h"
}

// Default value functions
fn default_auth_method() -> String {
    "auto".to_string()
}
fn default_true() -> bool {
    true
}
fn default_sync_strategy() -> String {
    "safe-pull".to_string()
}
fn default_max_parallel() -> usize {
    4
}
fn default_timeout() -> u64 {
    300
}
fn default_interval() -> String {
    "30m".to_string()
}
fn default_pid_file() -> String {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        format!("{}/reposentry.pid", runtime_dir)
    } else {
        "/tmp/reposentry.pid".to_string()
    }
}

fn default_log_file() -> String {
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        format!("{}/reposentry/daemon.log", data_home)
    } else if let Ok(home) = std::env::var("HOME") {
        format!("{}/.local/share/reposentry/daemon.log", home)
    } else {
        "/tmp/reposentry-daemon.log".to_string()
    }
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_log_format() -> String {
    "compact".to_string()
}
fn default_conflict_resolution() -> String {
    "prefix-org".to_string()
}
fn default_cache_duration() -> String {
    "1h".to_string()
}

// Default implementations
impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            age: AgeFilter {
                max_age: Some("3month".to_string()),
            },
            size: SizeFilter {
                max_size: Some("1GB".to_string()),
            },
        }
    }
}

impl Default for GitHubConfig {
    fn default() -> Self {
        Self {
            auth_method: default_auth_method(),
            username: None,
            include_organizations: default_true(),
            exclude_patterns: vec![
                "archived-*".to_string(),
                "test-*".to_string(),
                "*.github.io".to_string(),
                "fork-*".to_string(),
            ],
            include_forks: false,
        }
    }
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            strategy: default_sync_strategy(),
            max_parallel: default_max_parallel(),
            timeout: default_timeout(),
            auto_stash: false,
            fast_forward_only: default_true(),
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval: default_interval(),
            pid_file: default_pid_file(),
            log_file: default_log_file(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            color: default_true(),
        }
    }
}

impl Default for OrganizationConfig {
    fn default() -> Self {
        Self {
            separate_org_dirs: default_true(),
            conflict_resolution: default_conflict_resolution(),
        }
    }
}

impl Default for AdvancedConfig {
    fn default() -> Self {
        Self {
            preserve_timestamps: default_true(),
            verify_clone: default_true(),
            cleanup_on_error: default_true(),
            cache_duration: default_cache_duration(),
        }
    }
}

impl Config {
    /// Load configuration from the default location or create a default config
    pub fn load_or_default() -> Result<Self> {
        let config_path = Self::default_config_path()?;

        if config_path.exists() {
            Self::load(&config_path)
        } else {
            // Create default config
            let config = Self::default();

            // Create config directory if it doesn't exist
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
            }

            // Save default config
            config.save(&config_path)?;

            tracing::info!("Created default configuration at: {:?}", config_path);
            Ok(config)
        }
    }

    /// Load configuration from a specific file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {:?}", path))?;

        let mut config: Config = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {:?}", path))?;

        // Expand environment variables in paths
        config.expand_paths()?;

        Ok(config)
    }

    /// Save configuration to a file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_yaml::to_string(self).context("Failed to serialize configuration")?;

        std::fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {:?}", path))?;

        Ok(())
    }

    /// Get the default configuration file path (XDG compliant)
    pub fn default_config_path() -> Result<PathBuf> {
        let config_dir = config_dir().context("Failed to get user config directory")?;

        Ok(config_dir.join("reposentry").join("config.yml"))
    }

    /// Expand environment variables in configuration paths
    pub fn expand_paths(&mut self) -> Result<()> {
        self.base_directory = shellexpand::full(&self.base_directory)
            .context("Failed to expand base_directory path")?
            .into_owned();

        self.daemon.pid_file = shellexpand::full(&self.daemon.pid_file)
            .context("Failed to expand pid_file path")?
            .into_owned();

        self.daemon.log_file = shellexpand::full(&self.daemon.log_file)
            .context("Failed to expand log_file path")?
            .into_owned();

        Ok(())
    }

    /// Convert age filter string to chrono Duration for comparison
    pub fn age_filter_duration(&self) -> Option<Duration> {
        self.filters
            .age
            .max_age
            .as_ref()
            .and_then(|age_str| match age_str.as_str() {
                "1month" => Some(Duration::days(30)),
                "3month" => Some(Duration::days(90)),
                "6month" => Some(Duration::days(180)),
                _ => None,
            })
    }

    /// Convert size filter string to bytes for comparison
    pub fn size_filter_bytes(&self) -> Option<u64> {
        self.filters
            .size
            .max_size
            .as_ref()
            .and_then(|size_str| match size_str.as_str() {
                "100MB" => Some(100 * 1024 * 1024),
                "1GB" => Some(1024 * 1024 * 1024),
                _ => None,
            })
    }

    /// Check if a repository should be filtered based on age
    pub fn should_filter_by_age(&self, last_activity: &chrono::DateTime<Utc>) -> bool {
        if let Some(max_age) = self.age_filter_duration() {
            let cutoff_date = Utc::now() - max_age;
            last_activity < &cutoff_date
        } else {
            false
        }
    }

    /// Check if a repository should be filtered based on size
    pub fn should_filter_by_size(&self, size_bytes: u64) -> bool {
        if let Some(max_size) = self.size_filter_bytes() {
            size_bytes > max_size
        } else {
            false
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base_directory: "${HOME}/dev".to_string(),
            filters: FilterConfig::default(),
            github: GitHubConfig::default(),
            sync: SyncConfig::default(),
            daemon: DaemonConfig::default(),
            logging: LoggingConfig::default(),
            organization: OrganizationConfig::default(),
            advanced: AdvancedConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    // Helper function to create a temporary config directory
    fn setup_test_config_dir() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_dir = temp_dir.path().join("reposentry");
        std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");
        (temp_dir, config_dir)
    }

    #[test]
    fn test_config_default_values() {
        let config = Config::default();

        assert_eq!(config.base_directory, "${HOME}/dev");
        assert!(config.github.include_organizations);
        assert!(!config.github.include_forks);
        assert_eq!(config.sync.max_parallel, 4);
        assert_eq!(config.sync.timeout, 300);
        assert!(!config.sync.auto_stash);
        assert!(config.sync.fast_forward_only);
        assert!(!config.daemon.enabled);
        assert!(config.organization.separate_org_dirs);
        assert!(config.advanced.preserve_timestamps);
    }

    #[test]
    fn test_age_filter_duration() {
        let mut config = Config::default();

        // Test 1 month
        config.filters.age.max_age = Some("1month".to_string());
        assert_eq!(config.age_filter_duration().unwrap().num_days(), 30);

        // Test 3 months
        config.filters.age.max_age = Some("3month".to_string());
        assert_eq!(config.age_filter_duration().unwrap().num_days(), 90);

        // Test 6 months
        config.filters.age.max_age = Some("6month".to_string());
        assert_eq!(config.age_filter_duration().unwrap().num_days(), 180);

        // Test invalid
        config.filters.age.max_age = Some("invalid".to_string());
        assert!(config.age_filter_duration().is_none());

        // Test none
        config.filters.age.max_age = None;
        assert!(config.age_filter_duration().is_none());
    }

    #[test]
    fn test_size_filter_bytes() {
        let mut config = Config::default();

        // Test 100MB
        config.filters.size.max_size = Some("100MB".to_string());
        assert_eq!(config.size_filter_bytes().unwrap(), 100 * 1024 * 1024);

        // Test 1GB
        config.filters.size.max_size = Some("1GB".to_string());
        assert_eq!(config.size_filter_bytes().unwrap(), 1024 * 1024 * 1024);

        // Test invalid
        config.filters.size.max_size = Some("invalid".to_string());
        assert!(config.size_filter_bytes().is_none());

        // Test none
        config.filters.size.max_size = None;
        assert!(config.size_filter_bytes().is_none());
    }

    #[test]
    fn test_expand_paths() {
        // Set up test environment
        env::set_var("TEST_REPOSENTRY_HOME", "/test/home");

        let mut config = Config::default();
        config.base_directory = "${TEST_REPOSENTRY_HOME}/dev".to_string();

        config.expand_paths().expect("Failed to expand paths");

        assert_eq!(config.base_directory, "/test/home/dev");

        // Clean up
        env::remove_var("TEST_REPOSENTRY_HOME");
    }

    #[test]
    fn test_config_load_nonexistent_file() {
        let nonexistent_path = Path::new("/nonexistent/path/config.yml");
        let result = Config::load(nonexistent_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_save_and_load() {
        let (_temp_dir, config_dir) = setup_test_config_dir();
        let config_path = config_dir.join("config.yml");

        // Create a config with non-default values
        let mut config = Config::default();
        config.base_directory = "/custom/path".to_string();
        config.github.username = Some("testuser".to_string());
        config.filters.age.max_age = Some("1month".to_string());
        config.sync.max_parallel = 8;

        // Save the config
        config.save(&config_path).expect("Failed to save config");

        // Load it back
        let loaded_config = Config::load(&config_path).expect("Failed to load config");

        assert_eq!(loaded_config.base_directory, "/custom/path");
        assert_eq!(loaded_config.github.username, Some("testuser".to_string()));
        assert_eq!(loaded_config.filters.age.max_age, Some("1month".to_string()));
        assert_eq!(loaded_config.sync.max_parallel, 8);
    }

    #[test]
    fn test_config_default_path_xdg() {
        // This test verifies that the default path respects XDG directories
        let default_path = Config::default_config_path().expect("Failed to get default path");
        assert!(default_path.to_string_lossy().contains("reposentry"));
        assert!(default_path.to_string_lossy().ends_with("config.yml"));
    }

    #[test]
    fn test_filtering_methods() {
        let mut config = Config::default();
        config.filters.age.max_age = Some("3month".to_string());
        config.filters.size.max_size = Some("1GB".to_string());

        // Test age filtering duration
        assert_eq!(config.age_filter_duration().unwrap().num_days(), 90);

        // Test size filtering bytes
        assert_eq!(config.size_filter_bytes().unwrap(), 1024 * 1024 * 1024);

        // Test should_filter_by_age
        let old_timestamp = Utc::now() - Duration::days(100);
        let recent_timestamp = Utc::now() - Duration::days(30);

        assert!(config.should_filter_by_age(&old_timestamp));
        assert!(!config.should_filter_by_age(&recent_timestamp));

        // Test should_filter_by_size
        let large_size: u64 = 2 * 1024 * 1024 * 1024; // 2GB
        let small_size: u64 = 500 * 1024 * 1024; // 500MB

        assert!(config.should_filter_by_size(large_size));
        assert!(!config.should_filter_by_size(small_size));
    }

    #[test]
    fn test_yaml_parsing() {
        let yaml_content = r#"
base_directory: "${HOME}/custom-dev"
filters:
  age:
    max_age: "1month"
  size:
    max_size: "100MB"
github:
  auth_method: "gh_cli"
  username: "testuser"
  include_organizations: false
  include_forks: true
sync:
  max_parallel: 8
  timeout: 600
  auto_stash: true
  fast_forward_only: false
daemon:
  enabled: true
  interval: "1h"
logging:
  level: "debug"
  format: "json"
  color: false
organization:
  separate_org_dirs: false
advanced:
  preserve_timestamps: false
"#;

        let config: Config = serde_yaml::from_str(yaml_content).expect("Failed to parse YAML");

        assert_eq!(config.base_directory, "${HOME}/custom-dev");
        assert_eq!(config.filters.age.max_age, Some("1month".to_string()));
        assert_eq!(config.filters.size.max_size, Some("100MB".to_string()));
        assert_eq!(config.github.username, Some("testuser".to_string()));
        assert!(!config.github.include_organizations);
        assert!(config.github.include_forks);
        assert_eq!(config.sync.max_parallel, 8);
        assert_eq!(config.sync.timeout, 600);
        assert!(config.sync.auto_stash);
        assert!(!config.sync.fast_forward_only);
        assert!(config.daemon.enabled);
        assert_eq!(config.daemon.interval, "1h");
        assert!(!config.logging.color);
        assert!(!config.organization.separate_org_dirs);
        assert!(!config.advanced.preserve_timestamps);
    }
}
