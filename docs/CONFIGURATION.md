# RepoSentry Configuration Guide

This guide covers all configuration options for RepoSentry, including setup, customization, and advanced usage patterns.

## Table of Contents

- [Configuration File Location](#configuration-file-location)
- [Basic Configuration](#basic-configuration)
- [Repository Filtering](#repository-filtering)
- [GitHub Integration](#github-integration)
- [Synchronization Settings](#synchronization-settings)
- [Daemon Configuration](#daemon-configuration)
- [Environment Variables](#environment-variables)
- [Common Configuration Patterns](#common-configuration-patterns)
- [Troubleshooting](#troubleshooting)

## Configuration File Location

RepoSentry follows XDG Base Directory Specification for configuration:

- **Linux/macOS**: `~/.config/reposentry/config.yml`
- **Windows**: `%APPDATA%\reposentry\config.yml`

The configuration file is automatically created when you run `reposentry init`.

## Basic Configuration

### Complete Configuration Template

```yaml
# Base directory where repositories are stored
# Supports environment variable expansion
base_directory: "${HOME}/dev"

# Repository filtering options
filters:
  age:
    max_age: "3month"        # 1month, 3month, 6month, never
  size:
    max_size: "1GB"          # 100MB, 1GB, 10GB, unlimited
  exclude_forks: true        # Skip forked repositories
  exclude_archived: true    # Skip archived repositories
  include_private: true     # Include private repositories

# GitHub API integration
github:
  auth_method: "auto"       # auto, gh_cli, token
  include_organizations: true
  token: ""                 # GitHub personal access token (if not using gh CLI)
  exclude_patterns:
    - "archived-*"          # Exclude repos starting with "archived-"
    - "test-*"              # Exclude test repositories
    - "*-backup"            # Exclude backup repositories
    - "fork/*"              # Exclude all forks

# Synchronization behavior
sync:
  max_parallel: 6           # Concurrent operations (recommended: 4-8)
  timeout: 300              # Timeout per operation (seconds)
  strategy: "safe-pull"     # safe-pull, fetch-only, always-pull
  conflict_resolution: "skip"  # skip, fetch-only, interactive, force
  preserve_local_changes: true # Preserve uncommitted changes

# Daemon/background service configuration
daemon:
  interval: "30m"           # Sync interval: 15m, 30m, 1h, 2h, 4h, daily
  pid_file: "${HOME}/.local/share/reposentry/daemon.pid"
  log_file: ""              # Log file path (empty = stdout)
  max_log_size: "100MB"     # Log rotation size
  startup_delay: "5s"       # Delay before first sync

# Logging configuration
logging:
  level: "info"             # trace, debug, info, warn, error
  format: "pretty"          # pretty, json, compact
  file: ""                  # Log to file (empty = stdout/stderr)
```

### Minimal Configuration

For a simple setup, you only need:

```yaml
base_directory: "${HOME}/dev"
github:
  include_organizations: true
```

All other settings will use sensible defaults.

## Repository Filtering

### Age-Based Filtering

Control which repositories to sync based on their age:

```yaml
filters:
  age:
    max_age: "3month"  # Options: 1month, 3month, 6month, never
```

- `1month`: Only repositories updated in the last month
- `3month`: Repositories updated in the last 3 months
- `6month`: Repositories updated in the last 6 months
- `never`: Include all repositories regardless of age

### Size-Based Filtering

Filter repositories by their size to manage bandwidth and storage:

```yaml
filters:
  size:
    max_size: "1GB"  # Options: 100MB, 1GB, 10GB, unlimited
```

- `100MB`: Only repositories under 100 megabytes
- `1GB`: Repositories under 1 gigabyte
- `10GB`: Repositories under 10 gigabytes
- `unlimited`: No size restrictions

### Pattern-Based Exclusions

Use glob patterns to exclude specific repositories:

```yaml
github:
  exclude_patterns:
    - "archived-*"        # Exclude anything starting with "archived-"
    - "*-test"            # Exclude anything ending with "-test"
    - "temp/*"            # Exclude anything in "temp" organization
    - "backup-repo-*"     # Exclude backup repositories
    - "*.wiki"            # Exclude wiki repositories
```

### Repository Type Filtering

Control which types of repositories to include:

```yaml
filters:
  exclude_forks: true      # Skip forked repositories
  exclude_archived: true   # Skip archived repositories
  include_private: true    # Include private repositories (default: true)
  include_public: true     # Include public repositories (default: true)
```

## GitHub Integration

### Authentication Methods

RepoSentry supports multiple authentication methods:

#### Auto-Detection (Recommended)
```yaml
github:
  auth_method: "auto"  # Tries gh CLI first, then GITHUB_TOKEN
```

#### GitHub CLI
```yaml
github:
  auth_method: "gh_cli"  # Uses `gh auth token`
```

#### Personal Access Token
```yaml
github:
  auth_method: "token"
  token: "ghp_your_token_here"  # Or use GITHUB_TOKEN environment variable
```

### Organization Support

Include repositories from organizations you belong to:

```yaml
github:
  include_organizations: true  # Auto-discover and include org repos
  # Or specify specific organizations:
  organizations:
    - "my-company"
    - "my-team"
```

### Rate Limiting

Configure GitHub API rate limiting behavior:

```yaml
github:
  respect_rate_limits: true    # Wait when rate limited (default: true)
  requests_per_minute: 5000    # Max requests per minute (default: 5000)
```

## Synchronization Settings

### Concurrency Control

Configure how many repositories sync in parallel:

```yaml
sync:
  max_parallel: 6  # Recommended range: 4-8
```

**Guidelines:**
- **4-6**: Conservative, good for slower connections
- **6-8**: Optimal for most situations
- **8+**: Only for very fast connections and powerful machines

RepoSentry automatically reduces concurrency for:
- Large repositories (>50MB: 50% reduction)
- Many repositories (>50 repos: bandwidth optimization)

### Sync Strategies

Choose how RepoSentry handles repository updates:

```yaml
sync:
  strategy: "safe-pull"  # Options: safe-pull, fetch-only, always-pull
```

- **`safe-pull`** (Recommended): Pull only when safe, fetch otherwise
- **`fetch-only`**: Never modify working directory, only fetch updates
- **`always-pull`**: Always attempt to pull (may cause conflicts)

### Conflict Resolution

Define how to handle conflicts:

```yaml
sync:
  conflict_resolution: "skip"  # Options: skip, fetch-only, interactive, force
```

- **`skip`**: Skip repositories with conflicts
- **`fetch-only`**: Fetch updates without modifying working directory
- **`interactive`**: Prompt user for each conflict (daemon mode uses skip)
- **`force`**: Force pull, potentially losing local changes ⚠️

### Timeout Configuration

Control operation timeouts:

```yaml
sync:
  timeout: 300          # Per-repository timeout in seconds
  global_timeout: 3600  # Total sync operation timeout
```

## Daemon Configuration

### Sync Intervals

Configure how often the daemon syncs repositories:

```yaml
daemon:
  interval: "30m"  # Options: 15m, 30m, 1h, 2h, 4h, daily
```

**Recommended intervals:**
- **15m**: High-frequency development environments
- **30m**: Standard development workflow
- **1h**: Balanced performance and freshness
- **2h+**: Low-frequency or large repository sets

### Process Management

Configure daemon process management:

```yaml
daemon:
  pid_file: "${HOME}/.local/share/reposentry/daemon.pid"
  log_file: "${HOME}/.local/share/reposentry/daemon.log"
  startup_delay: "5s"      # Delay before first sync
  max_memory: "500MB"      # Memory limit (Linux only)
```

### Logging Configuration

Control daemon logging:

```yaml
logging:
  level: "info"            # trace, debug, info, warn, error
  format: "json"           # pretty, json, compact
  file: "/var/log/reposentry.log"
  max_size: "100MB"        # Log rotation size
  max_files: 5             # Number of rotated logs to keep
```

## Environment Variables

RepoSentry respects these environment variables:

### Authentication
- **`GITHUB_TOKEN`**: GitHub personal access token
- **`GH_TOKEN`**: Alternative GitHub token variable

### Configuration Override
- **`REPOSENTRY_CONFIG`**: Path to configuration file
- **`REPOSENTRY_BASE_DIR`**: Override base_directory setting
- **`REPOSENTRY_LOG_LEVEL`**: Override logging level

### Daemon Control
- **`REPOSENTRY_PID_FILE`**: Override daemon PID file location
- **`REPOSENTRY_DAEMON_INTERVAL`**: Override sync interval

### Example Environment Setup
```bash
export GITHUB_TOKEN="ghp_your_token_here"
export REPOSENTRY_LOG_LEVEL="debug"
export REPOSENTRY_BASE_DIR="/home/user/projects"
```

## Common Configuration Patterns

### Development Workstation
```yaml
base_directory: "${HOME}/code"
filters:
  max_age: "1month"        # Recent repositories only
  max_size: "1GB"          # Exclude very large repos
  exclude_forks: false     # Include forks for OSS contributions
sync:
  max_parallel: 8          # Fast development machine
  strategy: "safe-pull"    # Safe for active development
daemon:
  interval: "30m"          # Frequent updates
```

### CI/CD Server
```yaml
base_directory: "/var/lib/reposentry"
filters:
  exclude_forks: true      # Only canonical repositories
  exclude_archived: true   # Skip inactive projects
sync:
  max_parallel: 4          # Conservative for server
  strategy: "fetch-only"   # Never modify working directories
  timeout: 600             # Longer timeout for large repos
daemon:
  interval: "1h"           # Balanced frequency
logging:
  level: "warn"            # Minimal logging
  format: "json"           # Structured logs for parsing
```

### Backup/Archive System
```yaml
base_directory: "/backup/repositories"
filters:
  max_age: "never"         # Archive everything
  max_size: "unlimited"    # No size limits
  exclude_forks: false     # Include forks
sync:
  max_parallel: 2          # Gentle on storage system
  strategy: "fetch-only"   # Preserve all states
daemon:
  interval: "daily"        # Infrequent updates sufficient
```

### Laptop with Limited Bandwidth
```yaml
base_directory: "${HOME}/repos"
filters:
  max_size: "100MB"        # Small repositories only
  exclude_forks: true      # Reduce redundancy
sync:
  max_parallel: 2          # Bandwidth-conscious
  timeout: 900             # Longer timeout for slow connection
daemon:
  interval: "2h"           # Infrequent syncing
```

## Troubleshooting

### Common Issues

#### Configuration Not Found
```bash
# Check configuration location
reposentry doctor

# Create default configuration
reposentry init --force
```

#### Authentication Failures
```bash
# Test authentication
reposentry auth test

# Setup authentication
reposentry auth setup

# Check GitHub CLI
gh auth status
```

#### Permission Issues
```bash
# Check file permissions
ls -la ~/.config/reposentry/

# Fix permissions
chmod 600 ~/.config/reposentry/config.yml
```

#### Large Repository Timeouts
```yaml
# Increase timeouts for large repositories
sync:
  timeout: 900              # 15 minutes
  max_parallel: 2           # Reduce concurrency
```

#### Rate Limiting
```yaml
# Handle GitHub API rate limits
github:
  requests_per_minute: 1000  # Reduce request rate
sync:
  max_parallel: 2            # Lower concurrency
```

### Debug Mode

Enable debug logging for troubleshooting:

```bash
# Set environment variable
export RUST_LOG=debug

# Or in configuration
logging:
  level: "debug"
```

### Validation

Validate your configuration:

```bash
# Check configuration validity
reposentry doctor

# Test dry-run with your settings
reposentry sync --dry-run

# Verify daemon settings
reposentry daemon status
```

### Getting Help

- **Configuration Issues**: Run `reposentry doctor`
- **Authentication Problems**: Run `reposentry auth test`
- **Sync Issues**: Use `reposentry sync --dry-run` first
- **Daemon Problems**: Check `reposentry daemon status`

---

*For more detailed information, see the [main README](../README.md) and [PRD.md](../PRD.md).*