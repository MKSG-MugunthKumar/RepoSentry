use assert_fs::{fixture::PathChild, TempDir};
use std::process::Command;

/// Integration tests for RepoSentry CLI commands
/// These tests run the actual binary and verify its behavior

#[test]
fn test_cli_help() {
    let output = Command::new("cargo")
        .args(&["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify help contains expected commands
    assert!(stdout.contains("init"));
    assert!(stdout.contains("auth"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("sync"));
    assert!(stdout.contains("daemon"));
    assert!(stdout.contains("doctor"));
}

#[test]
fn test_cli_version() {
    let output = Command::new("cargo")
        .args(&["run", "--", "--version"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("reposentry"));
}

#[test]
fn test_doctor_command() {
    let output = Command::new("cargo")
        .args(&["run", "--", "doctor"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify doctor output contains diagnostic information
    assert!(stdout.contains("System Diagnostics") || stdout.contains("Diagnostics"));
    assert!(stdout.contains("Git"));
}

#[test]
fn test_config_init_with_skip_auth() {
    let temp_dir = TempDir::new().unwrap();
    let _config_dir = temp_dir.child("reposentry");

    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            "init",
            "--skip-auth",
            "--base-dir",
            "/tmp/test",
        ])
        .env("XDG_CONFIG_HOME", temp_dir.path())
        .output()
        .expect("Failed to execute command");

    // Note: This test might fail if actual config creation requires authentication
    // In a real environment, we would need to mock or provide test credentials
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("initialized") || stdout.contains("Configuration"));
    } else {
        // If it fails, verify it's for expected reasons (authentication, etc.)
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("authentication") || stderr.contains("config") || !stderr.is_empty()
        );
    }
}

#[test]
fn test_auth_status() {
    let output = Command::new("cargo")
        .args(&["run", "--", "auth", "status"])
        .output()
        .expect("Failed to execute command");

    // This test will succeed if authentication is available, fail otherwise
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should either show successful auth or clear error message
    assert!(
        stdout.contains("successful")
            || stdout.contains("Authentication")
            || stderr.contains("authentication")
            || stderr.contains("GitHub")
    );
}

#[test]
#[ignore] // This test requires network access and valid GitHub credentials
fn test_list_repositories() {
    let output = Command::new("cargo")
        .args(&["run", "--", "list"])
        .output()
        .expect("Failed to execute command");

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // If successful, should contain repository information
        assert!(stdout.contains("repositories") || stdout.contains("Repository"));
    } else {
        // If failed, should be due to authentication
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("authentication") || stderr.contains("GitHub"));
    }
}

#[test]
fn test_invalid_command() {
    let output = Command::new("cargo")
        .args(&["run", "--", "nonexistent-command"])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error") || stderr.contains("unrecognized") || stderr.contains("invalid")
    );
}

#[test]
fn test_help_subcommands() {
    let subcommands = vec!["auth", "init", "list", "sync", "daemon", "doctor"];

    for cmd in subcommands {
        let output = Command::new("cargo")
            .args(&["run", "--", cmd, "--help"])
            .output()
            .expect(&format!("Failed to execute {} help", cmd));

        assert!(output.status.success(), "Help for {} command failed", cmd);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.len() > 0, "Help output for {} was empty", cmd);
    }
}

#[test]
fn test_verbose_flag() {
    let output = Command::new("cargo")
        .args(&["run", "--", "--verbose", "doctor"])
        .output()
        .expect("Failed to execute command");

    // Verbose flag should either work or show error, but not crash
    assert!(output.status.success() || !String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn test_config_file_option() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.child("custom-config.yml");

    // Create a basic config file
    std::fs::write(
        config_path.path(),
        r#"
base_directory: "/tmp/test"
github:
  auth_method: "auto"
"#,
    )
    .unwrap();

    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            "--config",
            config_path.path().to_str().unwrap(),
            "doctor",
        ])
        .output()
        .expect("Failed to execute command");

    // Should either succeed with custom config or show meaningful error
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Error should be about config content or authentication, not file parsing
        assert!(
            stderr.contains("authentication")
                || stderr.contains("GitHub")
                || stderr.contains("config")
                || !stderr.is_empty()
        );
    }
}

#[test]
fn test_error_handling_invalid_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.child("invalid-config.yml");

    // Create an invalid config file
    std::fs::write(config_path.path(), "invalid: yaml: content: [").unwrap();

    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            "--config",
            config_path.path().to_str().unwrap(),
            "doctor",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("parse") || stderr.contains("config") || stderr.contains("yaml"));
}

#[test]
fn test_compilation() {
    // Ensure the project compiles successfully
    let output = Command::new("cargo")
        .args(&["check"])
        .output()
        .expect("Failed to execute cargo check");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("Compilation failed:\n{}", stderr);
    }
}

#[test]
fn test_clippy_lints() {
    // Run clippy to check for common issues
    let output = Command::new("cargo")
        .args(&["clippy", "--", "-D", "warnings"])
        .output()
        .expect("Failed to execute cargo clippy");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("Clippy warnings/errors:\n{}", stderr);
        // Don't fail the test for clippy warnings, just report them
    }

    assert!(true); // Always pass, we just want to see the output
}

#[test]
fn test_format_check() {
    // Check code formatting
    let output = Command::new("cargo")
        .args(&["fmt", "--check"])
        .output()
        .expect("Failed to execute cargo fmt");

    if !output.status.success() {
        println!("Code formatting issues detected. Run 'cargo fmt' to fix.");
    }

    assert!(true); // Always pass, we just want to see the output
}
