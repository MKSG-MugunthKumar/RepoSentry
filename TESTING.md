# RepoSentry Testing Guide

This document outlines testing procedures and validation steps for RepoSentry development.

## Quick Validation Test Suite

### 1. Build and Compilation Tests

```bash
# Check compilation without building
cargo check

# Full build with dependencies
cargo build

# Test with release optimizations
cargo build --release
```

### 2. CLI Interface Tests

```bash
# Test help system
cargo run -- --help
cargo run -- init --help
cargo run -- auth --help

# Test version information
cargo run -- --version
```

### 3. System Diagnostics

```bash
# Full system health check
cargo run -- doctor

# Test verbose logging
cargo run -- --verbose doctor
```

**Expected Output:**
- ✅ Git installation detected
- ✅ GitHub authentication successful
- ✅ Base directory accessible
- ✅ SSH keys found

### 4. Configuration Management

```bash
# Test configuration creation
rm -f ~/.config/reposentry/config.yml
cargo run -- init --skip-auth

# Verify configuration exists
ls -la ~/.config/reposentry/config.yml

# Test custom base directory
cargo run -- init --base-dir ~/test-repos --skip-auth
```

### 5. GitHub Authentication Tests

```bash
# Test authentication detection
cargo run -- auth status

# Test authentication setup guidance
GITHUB_TOKEN="" cargo run -- auth test

# Test GitHub CLI integration
cargo run -- auth test
```

**Expected Scenarios:**
- With `gh` CLI: Should detect and use automatically
- With `GITHUB_TOKEN`: Should use environment variable
- With neither: Should provide setup instructions

### 6. Repository Discovery Tests

```bash
# Test basic repository listing
cargo run -- list

# Test detailed repository information
cargo run -- list --details

# Test organization filtering
cargo run -- list --org YOUR_ORG_NAME

# Test with verbose logging
cargo run -- --verbose list
```

**Expected Results:**
- Repository count should be displayed
- Filtering should reduce result count
- Organization repositories should be included/excluded based on configuration

### 7. Configuration Filtering Tests

```bash
# Test age filtering by modifying config
# Edit ~/.config/reposentry/config.yml:
# filters:
#   age:
#     max_age: "1month"

cargo run -- list --details

# Test size filtering
# Edit config.yml:
# filters:
#   size:
#     max_size: "100MB"

cargo run -- list --details
```

### 8. Error Handling Tests

```bash
# Test with invalid authentication
GITHUB_TOKEN="invalid" cargo run -- list

# Test with network disconnected
# (Disconnect network) cargo run -- list

# Test with invalid configuration
echo "invalid: yaml: content" > ~/.config/reposentry/config.yml
cargo run -- list
```

## Development Testing Workflow

### Pre-commit Testing

```bash
# 1. Compilation check
cargo check

# 2. Code formatting
cargo fmt --check

# 3. Linting
cargo clippy -- -D warnings

# 4. Basic functionality test
cargo run -- doctor
cargo run -- auth status
cargo run -- list | head -5
```

### Integration Testing

```bash
# Test different authentication methods
# Method 1: GitHub CLI
gh auth status && cargo run -- auth test

# Method 2: Environment token
export GITHUB_TOKEN="your_token"
cargo run -- auth test

# Method 3: No authentication
unset GITHUB_TOKEN
gh auth logout
cargo run -- auth test  # Should show setup instructions
```

### Performance Testing

```bash
# Time repository discovery
time cargo run -- list >/dev/null

# Test with verbose logging to check API calls
RUST_LOG=debug cargo run -- list 2>debug.log
grep "Fetching" debug.log
```

### Cross-Platform Testing

```bash
# Linux-specific paths
ls ~/.config/reposentry/config.yml

# Test XDG variable handling
unset XDG_CONFIG_HOME
cargo run -- init --skip-auth

export XDG_CONFIG_HOME=/tmp/custom-config
cargo run -- init --skip-auth
ls $XDG_CONFIG_HOME/reposentry/config.yml
```

## Manual Test Scenarios

### Scenario 1: First-Time User Setup

```bash
# 1. Clean environment
rm -rf ~/.config/reposentry

# 2. Initialize without authentication
cargo run -- init --skip-auth

# 3. Set up authentication
cargo run -- auth setup

# 4. Test functionality
cargo run -- doctor
cargo run -- list
```

### Scenario 2: Organization Developer Workflow

```bash
# 1. List all repositories
cargo run -- list --details

# 2. Filter by organization
cargo run -- list --org COMPANY_NAME

# 3. Test exclusion patterns (modify config)
# exclude_patterns: ["archived-*", "test-*"]
cargo run -- list
```

### Scenario 3: Configuration Customization

```bash
# 1. Modify base directory
cargo run -- init --base-dir ~/custom-dev

# 2. Update filtering preferences
# Edit ~/.config/reposentry/config.yml
# filters:
#   age:
#     max_age: "6month"
#   size:
#     max_size: "1GB"

# 3. Test new configuration
cargo run -- list --details
```

## Known Test Issues and Workarounds

### Issue 1: Broken Pipe with `head` Command

```bash
# Problem:
cargo run -- list | head -5
# Results in: "Broken pipe" error

# Workaround:
timeout 5s cargo run -- list
# OR
cargo run -- list | wc -l  # Count repositories
```

### Issue 2: API Rate Limiting

```bash
# Problem: Too many API calls during development

# Solution: Use authentication to increase rate limits
export GITHUB_TOKEN="your_token"
cargo run -- auth test

# Monitor rate limits
RUST_LOG=debug cargo run -- list 2>&1 | grep -i rate
```

### Issue 3: Large Repository Collections

```bash
# Problem: Timeout during repository discovery

# Workaround: Test with organization filtering
cargo run -- list --org SMALL_ORG_NAME

# Monitor progress with verbose logging
cargo run -- --verbose list --org SPECIFIC_ORG
```

## Test Data Validation

### Expected Repository Counts

| Filter | Expected Behavior |
|--------|------------------|
| No filter | All accessible repositories |
| 1 month age | Only recently active repos |
| 100MB size | Small repositories only |
| Org filter | Only specified organization |
| Pattern exclusions | Filtered out matching patterns |

### Configuration File Validation

```bash
# Verify configuration structure
cat ~/.config/reposentry/config.yml

# Expected sections:
# - base_directory
# - filters (age, size)
# - github (auth_method, include_organizations, exclude_patterns)
# - sync, daemon, logging, organization, advanced
```

## Debugging Commands

### Logging Configuration

```bash
# Debug level logging
RUST_LOG=debug cargo run -- list

# Trace level (very verbose)
RUST_LOG=trace cargo run -- auth test

# Module-specific logging
RUST_LOG=reposentry::github=debug cargo run -- list
```

### Network Debugging

```bash
# Check GitHub API connectivity
curl -H "Authorization: token $GITHUB_TOKEN" https://api.github.com/user

# Verify GitHub CLI authentication
gh auth status -v

# Test SSH connectivity
ssh -T git@github.com
```

This testing framework ensures comprehensive validation of all implemented functionality while providing clear guidance for debugging issues and validating new features.