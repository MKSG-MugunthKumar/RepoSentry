# Contributing to RepoSentry

Thank you for your interest in contributing to RepoSentry! We welcome contributions from the community and are excited to see what you'll build with us.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Contributing Process](#contributing-process)
- [Code Standards](#code-standards)
- [Testing Guidelines](#testing-guidelines)
- [Documentation](#documentation)
- [Pull Request Process](#pull-request-process)
- [Issue Guidelines](#issue-guidelines)

## Code of Conduct

This project adheres to a code of conduct adapted from the Contributor Covenant. By participating, you are expected to uphold this code. Please report unacceptable behavior to the project maintainers.

**Our Standards:**
- Using welcoming and inclusive language
- Being respectful of differing viewpoints and experiences
- Gracefully accepting constructive criticism
- Focusing on what is best for the community
- Showing empathy towards other community members

## Getting Started

### Prerequisites

- **Rust 1.70+** with Cargo
- **Git** (any recent version)
- **GitHub CLI** (`gh`) or GitHub personal access token for testing
- **Linux/macOS/Windows** (cross-platform support)

### Fork and Clone

1. Fork the RepoSentry repository on GitHub
2. Clone your fork locally:
   ```bash
   git clone https://github.com/yourusername/RepoSentry.git
   cd RepoSentry
   ```

3. Add the upstream remote:
   ```bash
   git remote add upstream https://github.com/MKSG-MugunthKumar/RepoSentry.git
   ```

## Development Setup

### Build and Test

```bash
# Install dependencies and build
cargo build

# Run all tests
cargo test

# Check code formatting
cargo fmt --check

# Run clippy for linting
cargo clippy -- -D warnings

# Build release version
cargo build --release
```

### Development Environment

```bash
# Initialize development configuration
cargo run -- init --skip-auth

# Test system health
cargo run -- doctor

# Run in development mode with debug logging
RUST_LOG=debug cargo run -- sync --dry-run
```

## Contributing Process

### 1. Create an Issue First

Before starting work on a significant change:
- **Bug Reports**: Use the bug report template
- **Feature Requests**: Use the feature request template
- **Questions**: Use discussions for general questions

### 2. Branch Naming

Create a descriptive branch name:
```bash
# Feature branches
git checkout -b feature/add-gitlab-support
git checkout -b feature/hot-reload-config

# Bug fix branches
git checkout -b fix/daemon-pid-cleanup
git checkout -b fix/auth-token-validation

# Documentation branches
git checkout -b docs/update-configuration-guide
```

### 3. Development Guidelines

- **Write tests** for new functionality
- **Update documentation** for API changes
- **Follow Rust conventions** and use `cargo fmt`
- **Keep commits atomic** and write clear commit messages
- **Test cross-platform** if possible (Linux/macOS/Windows)

## Code Standards

### Rust Code Style

We follow standard Rust conventions:

```rust
// Use descriptive variable names
let repository_count = repositories.len();

// Document public APIs
/// Analyzes repository state to determine sync strategy
pub async fn analyze_repo_state(&self, path: &Path, remote_url: &str) -> Result<RepoState> {
    // Implementation
}

// Use Result types for error handling
fn parse_config(path: &Path) -> Result<Config> {
    // Never use unwrap() in production code
    let content = fs::read_to_string(path)
        .context("Failed to read configuration file")?;
    // ...
}

// Prefer explicit error handling
match auth_result {
    Ok(client) => client,
    Err(e) => {
        tracing::warn!("Authentication failed: {}", e);
        return Err(e);
    }
}
```

### Error Handling

- Use `anyhow::Result` for error propagation
- Provide **actionable error messages** with context
- Use `tracing` for logging (not `println!`)
- Test error conditions in unit tests

### Performance Considerations

- Use async/await for I/O operations
- Implement proper semaphore-based concurrency limits
- Consider bandwidth and API rate limiting
- Profile memory usage for large repository sets

## Testing Guidelines

### Unit Tests

Write unit tests for core logic:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parsing() {
        let config_yaml = r#"
base_directory: "/tmp/test"
github:
  include_organizations: true
"#;
        let config: Config = serde_yaml::from_str(config_yaml).unwrap();
        assert_eq!(config.base_directory, "/tmp/test");
    }

    #[tokio::test]
    async fn test_sync_engine_creation() {
        let config = Config::default();
        // Note: This may fail without authentication in CI
        match SyncEngine::new(config).await {
            Ok(_) => {}, // Success case
            Err(e) if e.to_string().contains("authentication") => {
                // Expected in CI without GitHub auth
            }
            Err(e) => panic!("Unexpected error: {}", e),
        }
    }
}
```

### Integration Tests

Integration tests should:
- Use temporary directories
- Mock external APIs when possible
- Test real GitHub API with authentication (when available)
- Verify cross-platform compatibility

### Testing Commands

```bash
# Run specific test module
cargo test config::tests

# Run with output
cargo test -- --nocapture

# Run integration tests
cargo test --test integration_tests

# Test in release mode
cargo test --release

# Test with specific features
cargo test --features "experimental"
```

## Documentation

### Code Documentation

- **Public APIs**: Must have doc comments with examples
- **Complex algorithms**: Explain the approach and trade-offs
- **Configuration**: Document all configuration options
- **Error messages**: Should guide users to solutions

### User Documentation

- Update `README.md` for user-facing changes
- Update `docs/CONFIGURATION.md` for configuration changes
- Add examples for new features
- Include troubleshooting information

### Commit Messages

Use conventional commit format:

```
feat: add GitLab backend support

- Implement GitLab API client with authentication
- Add configuration options for GitLab integration
- Update CLI commands to support --provider flag
- Add comprehensive tests for GitLab functionality

Closes #123
```

**Types:**
- `feat:` New features
- `fix:` Bug fixes
- `docs:` Documentation updates
- `test:` Test additions or updates
- `refactor:` Code refactoring
- `perf:` Performance improvements
- `chore:` Maintenance tasks

## Pull Request Process

### Before Submitting

1. **Sync with upstream:**
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Run full test suite:**
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

3. **Update documentation** if needed

4. **Verify cross-platform compatibility** if possible

### PR Requirements

- [ ] **Tests pass** in CI
- [ ] **Code follows style guidelines**
- [ ] **Documentation updated** for user-facing changes
- [ ] **Breaking changes documented** in commit message and PR description
- [ ] **Issue linked** in PR description (if applicable)

### PR Template

```markdown
## Description
Brief description of changes and motivation.

## Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to change)
- [ ] Documentation update

## Testing
- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] Manual testing completed
- [ ] Cross-platform testing (if applicable)

## Checklist
- [ ] Code follows project style guidelines
- [ ] Self-review completed
- [ ] Documentation updated
- [ ] Tests added for new functionality
- [ ] All CI checks pass

Closes #(issue_number)
```

## Issue Guidelines

### Bug Reports

**Include:**
- RepoSentry version (`reposentry --version`)
- Operating system and version
- Rust version (`rustc --version`)
- Clear steps to reproduce
- Expected vs actual behavior
- Relevant logs (with `RUST_LOG=debug`)

### Feature Requests

**Include:**
- Clear description of the problem or use case
- Proposed solution or implementation approach
- Alternative solutions considered
- Impact assessment (breaking changes, performance, etc.)

### Security Issues

**Do NOT** open public issues for security vulnerabilities.
Instead, email security issues to: [security@mk.sg]

## Development Areas

### High-Impact Contributions

- **GitLab Support**: Backend abstraction is ready, need GitLab API implementation
- **Configuration Hot-Reload**: IPC mechanism for runtime config updates
- **GUI Integration**: TUI/GUI frontend with ratatui or tauri
- **Advanced Filtering**: More sophisticated repository filtering options
- **Performance Optimizations**: Better concurrency and memory management

### Good First Issues

- Documentation improvements
- Adding more configuration validation
- Improving error messages
- Adding unit tests for existing functionality
- Cross-platform compatibility fixes

### Architecture Areas

- **Backend Abstraction**: `src/api/` for multiple git hosting providers
- **Config Management**: Hot-reload and validation improvements
- **Sync Strategies**: More sophisticated sync decision logic
- **Monitoring/Observability**: Metrics and monitoring integration

## Getting Help

- **General Questions**: Use GitHub Discussions
- **Bug Reports**: Create an issue with the bug report template
- **Feature Ideas**: Create an issue with the feature request template
- **Code Questions**: Comment on relevant issues or PRs
- **Real-time Chat**: (Link to Discord/Slack if available)

## Recognition

Contributors will be:
- Listed in `CONTRIBUTORS.md`
- Mentioned in release notes for significant contributions
- Invited to join the maintainers team for sustained contributions

---

**Thank you for contributing to RepoSentry!** 🚀

Your contributions help build better developer tools for the entire community.