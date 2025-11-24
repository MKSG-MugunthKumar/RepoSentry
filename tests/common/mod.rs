/// Common test utilities and helpers for RepoSentry tests

use std::env;
use std::path::PathBuf;
use tempfile::TempDir;

/// Test configuration helper
pub struct TestEnvironment {
    pub temp_dir: TempDir,
    pub config_dir: PathBuf,
    pub original_env: Vec<(String, Option<String>)>,
}

impl TestEnvironment {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_dir = temp_dir.path().join("reposentry");
        std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");

        // Store original environment variables
        let env_vars = vec!["GITHUB_TOKEN", "XDG_CONFIG_HOME", "HOME"];
        let original_env = env_vars
            .iter()
            .map(|var| (var.to_string(), env::var(var).ok()))
            .collect();

        Self {
            temp_dir,
            config_dir,
            original_env,
        }
    }

    pub fn set_env_var(&self, key: &str, value: &str) {
        env::set_var(key, value);
    }

    pub fn set_config_dir(&self) {
        env::set_var("XDG_CONFIG_HOME", self.temp_dir.path());
    }

    pub fn create_test_config(&self, content: &str) -> PathBuf {
        let config_path = self.config_dir.join("config.yml");
        std::fs::write(&config_path, content).expect("Failed to write test config");
        config_path
    }

    pub fn create_minimal_config(&self) -> PathBuf {
        let config_content = r#"
base_directory: "${HOME}/test-dev"
github:
  auth_method: "auto"
  include_organizations: true
  include_forks: false
filters:
  age:
    max_age: "3month"
  size:
    max_size: "1GB"
"#;
        self.create_test_config(config_content)
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        // Restore original environment variables
        for (key, value) in &self.original_env {
            match value {
                Some(val) => env::set_var(key, val),
                None => env::remove_var(key),
            }
        }
    }
}

/// Mock GitHub repository data for testing
#[derive(Debug, Clone)]
pub struct MockRepository {
    pub name: String,
    pub full_name: String,
    pub size_kb: u64,
    pub updated_days_ago: u32,
    pub is_fork: bool,
    pub is_private: bool,
}

impl MockRepository {
    pub fn new(name: &str, owner: &str) -> Self {
        Self {
            name: name.to_string(),
            full_name: format!("{}/{}", owner, name),
            size_kb: 1000,
            updated_days_ago: 30,
            is_fork: false,
            is_private: false,
        }
    }

    pub fn with_size_mb(mut self, size_mb: u64) -> Self {
        self.size_kb = size_mb * 1024;
        self
    }

    pub fn with_age_days(mut self, days: u32) -> Self {
        self.updated_days_ago = days;
        self
    }

    pub fn as_fork(mut self) -> Self {
        self.is_fork = true;
        self
    }

    pub fn as_private(mut self) -> Self {
        self.is_private = true;
        self
    }
}

/// Test data sets for common scenarios
pub struct TestDataSets;

impl TestDataSets {
    /// Create a set of repositories for testing filtering
    pub fn mixed_repositories() -> Vec<MockRepository> {
        vec![
            MockRepository::new("active-project", "user")
                .with_size_mb(500)
                .with_age_days(15),
            MockRepository::new("old-project", "user")
                .with_size_mb(200)
                .with_age_days(120),
            MockRepository::new("large-project", "user")
                .with_size_mb(2000)
                .with_age_days(30),
            MockRepository::new("fork-project", "user")
                .with_size_mb(100)
                .with_age_days(10)
                .as_fork(),
            MockRepository::new("private-project", "user")
                .with_size_mb(300)
                .with_age_days(20)
                .as_private(),
            MockRepository::new("test-archived-repo", "org")
                .with_size_mb(50)
                .with_age_days(200),
        ]
    }

    /// Create organization repositories
    pub fn organization_repositories() -> Vec<MockRepository> {
        vec![
            MockRepository::new("main-product", "company"),
            MockRepository::new("internal-tools", "company"),
            MockRepository::new("public-api", "company"),
        ]
    }
}

/// Assertion helpers for test validation
pub fn assert_contains_all(text: &str, expected: &[&str]) {
    for item in expected {
        assert!(
            text.contains(item),
            "Expected text to contain '{}', but it didn't. Text: {}",
            item,
            text
        );
    }
}

pub fn assert_contains_any(text: &str, expected: &[&str]) {
    let found = expected.iter().any(|item| text.contains(item));
    assert!(
        found,
        "Expected text to contain at least one of {:?}, but it didn't. Text: {}",
        expected,
        text
    );
}

/// Test timeout helper for network operations
pub use std::time::{Duration, Instant};

pub fn with_timeout<F, R>(duration: Duration, f: F) -> Option<R>
where
    F: FnOnce() -> R,
{
    let start = Instant::now();
    let result = f();

    if start.elapsed() < duration {
        Some(result)
    } else {
        None
    }
}