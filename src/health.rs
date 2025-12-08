//! System health checks for RepoSentry
//!
//! This module provides preflight checks to verify the system is properly
//! configured before running operations.

use crate::{Config, GitHubClient};
use std::path::Path;

/// Result of system health checks
#[derive(Debug, Clone)]
pub struct HealthCheck {
    /// Git installation status
    pub git: CheckResult,
    /// GitHub authentication status
    pub github_auth: CheckResult,
    /// Base directory status
    pub base_dir: CheckResult,
    /// SSH configuration status (warning only, not required)
    pub ssh: CheckResult,
}

/// Result of an individual health check
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub passed: bool,
    pub message: String,
    pub details: Option<String>,
    pub is_warning: bool,
}

#[allow(dead_code)]
impl CheckResult {
    fn ok(message: impl Into<String>) -> Self {
        Self {
            passed: true,
            message: message.into(),
            details: None,
            is_warning: false,
        }
    }

    fn ok_with_details(message: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            passed: true,
            message: message.into(),
            details: Some(details.into()),
            is_warning: false,
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            passed: false,
            message: message.into(),
            details: None,
            is_warning: false,
        }
    }

    fn error_with_details(message: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            passed: false,
            message: message.into(),
            details: Some(details.into()),
            is_warning: false,
        }
    }

    fn warning(message: impl Into<String>) -> Self {
        Self {
            passed: true,
            message: message.into(),
            details: None,
            is_warning: true,
        }
    }

    fn warning_with_details(message: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            passed: true,
            message: message.into(),
            details: Some(details.into()),
            is_warning: true,
        }
    }
}

impl HealthCheck {
    /// Run all health checks
    pub async fn run(config: &Config) -> Self {
        Self {
            git: Self::check_git(),
            github_auth: Self::check_github_auth(config).await,
            base_dir: Self::check_base_dir(config),
            ssh: Self::check_ssh(),
        }
    }

    /// Check if all required checks passed (excludes warnings)
    pub fn all_passed(&self) -> bool {
        self.git.passed && self.github_auth.passed && self.base_dir.passed
        // SSH is optional, not included in required checks
    }

    /// Get list of failed checks (errors only, not warnings)
    pub fn errors(&self) -> Vec<&CheckResult> {
        [&self.git, &self.github_auth, &self.base_dir, &self.ssh]
            .into_iter()
            .filter(|r| !r.passed && !r.is_warning)
            .collect()
    }

    /// Get list of warnings
    pub fn warnings(&self) -> Vec<&CheckResult> {
        [&self.git, &self.github_auth, &self.base_dir, &self.ssh]
            .into_iter()
            .filter(|r| r.is_warning)
            .collect()
    }

    /// Check git installation
    fn check_git() -> CheckResult {
        match std::process::Command::new("git").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                CheckResult::ok_with_details("Git installed", version.trim().to_string())
            }
            Ok(_) => CheckResult::error("Git command failed"),
            Err(_) => CheckResult::error_with_details(
                "Git not found in PATH",
                "Install git: https://git-scm.com/downloads",
            ),
        }
    }

    /// Check GitHub authentication
    async fn check_github_auth(config: &Config) -> CheckResult {
        match GitHubClient::new(config).await {
            Ok(client) => CheckResult::ok_with_details(
                "GitHub authentication successful",
                format!("Username: {}", client.username()),
            ),
            Err(e) => CheckResult::error_with_details(
                "GitHub authentication failed",
                format!("{}\nRun: gh auth login", e),
            ),
        }
    }

    /// Check base directory exists
    fn check_base_dir(config: &Config) -> CheckResult {
        match shellexpand::full(&config.base_directory) {
            Ok(expanded) => {
                let path = Path::new(expanded.as_ref());
                if path.exists() {
                    CheckResult::ok_with_details("Base directory exists", expanded.to_string())
                } else {
                    CheckResult::error_with_details(
                        "Base directory does not exist",
                        format!("Run: mkdir -p {}", expanded),
                    )
                }
            }
            Err(e) => CheckResult::error_with_details(
                "Invalid base directory path",
                e.to_string(),
            ),
        }
    }

    /// Check SSH configuration (warning only)
    fn check_ssh() -> CheckResult {
        let ssh_dir = dirs::home_dir().unwrap_or_default().join(".ssh");
        if !ssh_dir.exists() {
            return CheckResult::warning_with_details(
                "~/.ssh directory not found",
                "SSH cloning may not work. Run: ssh-keygen -t ed25519",
            );
        }

        let ssh_keys = ["id_rsa", "id_ed25519", "id_ecdsa"];
        let found_keys: Vec<_> = ssh_keys
            .iter()
            .filter(|key| ssh_dir.join(key).exists())
            .map(|s| *s)
            .collect();

        if found_keys.is_empty() {
            CheckResult::warning_with_details(
                "No SSH keys found",
                "SSH cloning may not work. Run: ssh-keygen -t ed25519 -C \"your_email@example.com\"",
            )
        } else {
            CheckResult::ok_with_details("SSH keys found", found_keys.join(", "))
        }
    }

    /// Get all checks as a slice for iteration
    pub fn all_checks(&self) -> [(&'static str, &CheckResult); 4] {
        [
            ("Git Installation", &self.git),
            ("GitHub Authentication", &self.github_auth),
            ("Base Directory", &self.base_dir),
            ("SSH Configuration", &self.ssh),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_result_ok() {
        let result = CheckResult::ok("Test passed");
        assert!(result.passed);
        assert!(!result.is_warning);
        assert!(result.details.is_none());
    }

    #[test]
    fn test_check_result_ok_with_details() {
        let result = CheckResult::ok_with_details("Test passed", "Some details");
        assert!(result.passed);
        assert!(!result.is_warning);
        assert_eq!(result.details, Some("Some details".to_string()));
    }

    #[test]
    fn test_check_result_warning() {
        let result = CheckResult::warning("Test warning");
        assert!(result.passed); // Warnings still "pass"
        assert!(result.is_warning);
    }

    #[test]
    fn test_check_result_warning_with_details() {
        let result = CheckResult::warning_with_details("Test warning", "Warning details");
        assert!(result.passed);
        assert!(result.is_warning);
        assert_eq!(result.details, Some("Warning details".to_string()));
    }

    #[test]
    fn test_check_result_error() {
        let result = CheckResult::error("Test failed");
        assert!(!result.passed);
        assert!(!result.is_warning);
    }

    #[test]
    fn test_check_result_error_with_details() {
        let result = CheckResult::error_with_details("Test failed", "Error details");
        assert!(!result.passed);
        assert!(!result.is_warning);
        assert_eq!(result.details, Some("Error details".to_string()));
    }

    #[test]
    fn test_git_check() {
        let result = HealthCheck::check_git();
        // Git should be installed in dev environment
        assert!(result.passed);
        assert!(result.details.is_some()); // Should have version info
    }

    #[test]
    fn test_check_base_dir_existing() {
        let mut config = Config::default();
        config.base_directory = "/tmp".to_string();
        let result = HealthCheck::check_base_dir(&config);
        assert!(result.passed);
        assert!(!result.is_warning);
    }

    #[test]
    fn test_check_base_dir_nonexistent() {
        let mut config = Config::default();
        config.base_directory = "/nonexistent/path/that/does/not/exist".to_string();
        let result = HealthCheck::check_base_dir(&config);
        assert!(!result.passed);
        assert!(result.details.is_some()); // Should have suggestion to create dir
    }

    #[test]
    fn test_check_base_dir_with_env_expansion() {
        let mut config = Config::default();
        // HOME should always be set
        config.base_directory = "$HOME".to_string();
        let result = HealthCheck::check_base_dir(&config);
        assert!(result.passed);
    }

    #[test]
    fn test_check_ssh() {
        let result = HealthCheck::check_ssh();
        // Result depends on system, but should not error
        assert!(result.passed || result.is_warning);
    }

    #[test]
    fn test_all_passed_with_passing_checks() {
        let health = HealthCheck {
            git: CheckResult::ok("Git OK"),
            github_auth: CheckResult::ok("Auth OK"),
            base_dir: CheckResult::ok("Dir OK"),
            ssh: CheckResult::warning("SSH warning"), // Warnings don't fail
        };
        assert!(health.all_passed());
    }

    #[test]
    fn test_all_passed_with_failing_git() {
        let health = HealthCheck {
            git: CheckResult::error("Git missing"),
            github_auth: CheckResult::ok("Auth OK"),
            base_dir: CheckResult::ok("Dir OK"),
            ssh: CheckResult::ok("SSH OK"),
        };
        assert!(!health.all_passed());
    }

    #[test]
    fn test_all_passed_with_failing_auth() {
        let health = HealthCheck {
            git: CheckResult::ok("Git OK"),
            github_auth: CheckResult::error("Auth failed"),
            base_dir: CheckResult::ok("Dir OK"),
            ssh: CheckResult::ok("SSH OK"),
        };
        assert!(!health.all_passed());
    }

    #[test]
    fn test_all_passed_with_failing_base_dir() {
        let health = HealthCheck {
            git: CheckResult::ok("Git OK"),
            github_auth: CheckResult::ok("Auth OK"),
            base_dir: CheckResult::error("Dir missing"),
            ssh: CheckResult::ok("SSH OK"),
        };
        assert!(!health.all_passed());
    }

    #[test]
    fn test_all_passed_with_ssh_warning() {
        // SSH warnings should NOT cause all_passed to fail
        let health = HealthCheck {
            git: CheckResult::ok("Git OK"),
            github_auth: CheckResult::ok("Auth OK"),
            base_dir: CheckResult::ok("Dir OK"),
            ssh: CheckResult::warning("No SSH keys"),
        };
        assert!(health.all_passed());
    }

    #[test]
    fn test_errors_returns_only_errors() {
        let health = HealthCheck {
            git: CheckResult::error("Git error"),
            github_auth: CheckResult::ok("Auth OK"),
            base_dir: CheckResult::error("Dir error"),
            ssh: CheckResult::warning("SSH warning"),
        };
        let errors = health.errors();
        assert_eq!(errors.len(), 2);
        assert!(!errors[0].passed);
        assert!(!errors[1].passed);
    }

    #[test]
    fn test_errors_excludes_warnings() {
        let health = HealthCheck {
            git: CheckResult::ok("Git OK"),
            github_auth: CheckResult::ok("Auth OK"),
            base_dir: CheckResult::ok("Dir OK"),
            ssh: CheckResult::warning("SSH warning"),
        };
        let errors = health.errors();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_warnings_returns_only_warnings() {
        let health = HealthCheck {
            git: CheckResult::ok("Git OK"),
            github_auth: CheckResult::error("Auth error"),
            base_dir: CheckResult::ok("Dir OK"),
            ssh: CheckResult::warning("SSH warning"),
        };
        let warnings = health.warnings();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].is_warning);
    }

    #[test]
    fn test_all_checks_returns_all_four() {
        let health = HealthCheck {
            git: CheckResult::ok("Git OK"),
            github_auth: CheckResult::ok("Auth OK"),
            base_dir: CheckResult::ok("Dir OK"),
            ssh: CheckResult::ok("SSH OK"),
        };
        let checks = health.all_checks();
        assert_eq!(checks.len(), 4);
        assert_eq!(checks[0].0, "Git Installation");
        assert_eq!(checks[1].0, "GitHub Authentication");
        assert_eq!(checks[2].0, "Base Directory");
        assert_eq!(checks[3].0, "SSH Configuration");
    }
}
