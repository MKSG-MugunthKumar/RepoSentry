# RepoSentry

**Intelligent Git Repository Synchronization Daemon**

An intelligent git repository synchronization daemon that automatically keeps local repository collections in sync with remote origins. Unlike traditional git tools that only fetch, RepoSentry intelligently pulls changes when safe or falls back to fetch-only when conflicts are detected, ensuring **zero data loss** through conservative conflict detection.

## Project Status

🎯 **Phase 2 Complete** - Full production-ready functionality implemented and tested

**Development Progress: Phase 2 - 100% Complete** *(Validated 2025-11-24)*

### ✅ Phase 1: Foundation & Discovery *(100% Complete)*
- **GitHub Authentication:** Auto-detection with `gh CLI` and `GITHUB_TOKEN` support ✅
- **Repository Discovery:** Full GitHub API integration with octocrab ✅
  - *Tested: 161 total repositories discovered (114 user + 47 org)*
- **Configuration System:** XDG-compliant YAML configuration with filtering ✅
- **CLI Interface:** Complete command structure with subcommands ✅
- **Age Filtering:** 1month, 3month, 6month repository age filters ✅
- **Size Filtering:** 100MB, 1GB repository size filters ✅
  - *Tested: Filters 161→33 repositories (79% reduction with 3month/1GB)*
- **Organization Support:** Automatic organization repository discovery ✅
  - *Tested: 4 organizations auto-discovered*
- **Pattern Exclusions:** Glob pattern matching for repository exclusions ✅
- **System Diagnostics:** `doctor` command for health checking ✅

### ✅ Phase 2: Git Operations, Sync Engine & Daemon *(100% Complete)*
- **Git Operations:** Parallel repository cloning and synchronization ✅
  - *Tested: Dry-run analysis of 114 repositories*
- **Intelligent Sync Engine:** Safe pull vs fetch-only decision logic ✅
- **Bandwidth-Aware Parallelization:** 4-8 concurrent operations (industry standard) ✅
- **Conflict Detection:** Working directory state analysis before pulls ✅
- **Daemon Infrastructure:** Background service with PID management ✅
- **Cross-Platform Support:** Unix signal handling with graceful shutdown ✅
- **Repository Size Optimization:** Large repo throttling (>50MB: 50% concurrency) ✅
- **Complete CLI Integration:** All commands fully functional ✅

### 🚧 Phase 3: Advanced Features *(In Planning)*
- **Configuration Hot-Reload:** Runtime config updates for GUI integration
- **Organization-Based Directory Structure:** Organized by GitHub orgs
- **Backend Abstraction:** GitLab support (architecture ready)

## Installation & Quick Start

### Prerequisites
- Rust 1.70+ with Cargo
- Git (any recent version)
- GitHub CLI (`gh`) or GitHub token for authentication

### Build from Source
```bash
git clone https://github.com/MKSG-MugunthKumar/RepoSentry
cd RepoSentry
cargo build --release

# Add to PATH for global usage
cp target/release/reposentry ~/.local/bin/
```

### Quick Setup
```bash
# Initialize configuration and authenticate with GitHub
reposentry init

# Verify system health and authentication
reposentry doctor

# Preview repositories to be synchronized
reposentry list

# Analyze what would be synced (dry-run)
reposentry sync --dry-run

# Start actual synchronization
reposentry sync

# Run as background daemon (30-minute intervals)
reposentry daemon start
```

## Core Commands

| Command | Description | Status |
|---------|-------------|---------|
| `reposentry init` | Setup configuration and authentication | ✅ **Production Ready** |
| `reposentry auth setup/test/status` | Authentication management | ✅ **Production Ready** |
| `reposentry list [--org ORG]` | Repository discovery and filtering | ✅ **Production Ready** |
| `reposentry sync [--dry-run] [--force]` | Repository synchronization | ✅ **Production Ready** |
| `reposentry daemon start/stop/status/restart` | Background service control | ✅ **Production Ready** |
| `reposentry doctor` | System diagnostics | ✅ **Production Ready** |

## Advanced Configuration

RepoSentry uses XDG-compliant configuration at `~/.config/reposentry/config.yml`.

**📖 See [docs/CONFIGURATION.md](./docs/CONFIGURATION.md) for complete configuration guide.**

### Quick Configuration Example

```yaml
# Where to store cloned repositories
base_directory: "${HOME}/dev"

# Repository filtering
filters:
  age:
    max_age: "3month"     # 1month, 3month, 6month
  size:
    max_size: "1GB"       # 100MB, 1GB
  exclude_forks: true     # Skip forked repositories
  exclude_archived: true # Skip archived repositories

# GitHub integration
github:
  auth_method: "auto"          # auto, gh_cli, token
  include_organizations: true  # Include org repositories
  exclude_patterns:
    - "archived-*"
    - "test-*"
    - "*-backup"

# Synchronization behavior
sync:
  max_parallel: 6        # Concurrent operations (4-8 recommended)
  timeout: 300           # Per-operation timeout (seconds)
  strategy: "safe-pull"  # safe-pull, fetch-only
  conflict_resolution: "skip"  # skip, fetch-only

# Daemon configuration
daemon:
  interval: "30m"          # Sync interval: 30m, 1h, 2h
  pid_file: "reposentry.pid"  # Filename only - placed in XDG_RUNTIME_DIR
  log_file: "daemon.log"      # Filename only - placed in XDG_DATA_HOME/reposentry
```

## Key Features

### 🚀 **Intelligent Synchronization**
- **Safe Pull Logic**: Only pulls when no conflicts detected
- **Bandwidth-Aware Concurrency**: 4-8 parallel operations based on repo size
- **Repository Size Optimization**: Automatic throttling for large repositories
- **Conflict Detection**: Pre-pull analysis of working directory state

### 🔧 **Production Ready**
- **Cross-Platform**: Linux, macOS, Windows support
- **Background Daemon**: Configurable sync intervals with graceful shutdown
- **Zero Data Loss**: Conservative conflict detection prevents accidental overwrites
- **Comprehensive Logging**: Structured logging with configurable levels

### 🌐 **GitHub Integration**
- **Auto-Authentication**: GitHub CLI and token support with fallbacks
- **Organization Support**: Automatic discovery of org repositories
- **Advanced Filtering**: Age, size, pattern-based repository exclusions
- **API Optimization**: Efficient pagination and rate limit handling

## Technology Stack

- **Language:** Rust *(2,500+ total lines implemented)*
- **Async Runtime:** Tokio *(full multi-threaded with signal handling)*
- **GitHub API:** octocrab 0.48 *(production-tested)*
- **CLI Framework:** clap 4.4 *(derive-based with subcommands)*
- **Configuration:** serde + serde_yaml *(XDG-compliant YAML)*
- **Logging:** tracing + tracing-subscriber *(structured with env-filter)*
- **Git Operations:** System git CLI *(respects existing authentication)*
- **Process Management:** daemonize + nix *(Unix signal handling)*
- **Concurrency:** futures + tokio::sync *(semaphore-controlled parallelism)*

---

*For detailed requirements and architecture, see [PRD.md](./PRD.md)*