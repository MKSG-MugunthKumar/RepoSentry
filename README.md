# RepoSentry

**Intelligent Git Repository Synchronization Daemon**

An intelligent git repository synchronization daemon that automatically keeps local repository collections in sync with remote origins. Unlike traditional git tools that only fetch, RepoSentry intelligently pulls changes when safe or falls back to fetch-only when conflicts are detected.

## Project Status

🚀 **MVP 1.0 Active Development** - Core functionality implemented and working

**Phase 1 Progress: 85% Complete**

### ✅ Completed Features
- **GitHub Authentication:** Auto-detection with `gh CLI` and `GITHUB_TOKEN` support
- **Repository Discovery:** Full GitHub API integration with octocrab
- **Configuration System:** XDG-compliant YAML configuration with filtering
- **CLI Interface:** Complete command structure with subcommands
- **Age Filtering:** 1month, 3month, 6month repository age filters
- **Size Filtering:** 100MB, 1GB repository size filters
- **Organization Support:** Automatic organization repository discovery
- **Pattern Exclusions:** Glob pattern matching for repository exclusions
- **System Diagnostics:** `doctor` command for health checking

### 🚧 In Progress
- **Git Operations:** Parallel repository cloning and syncing (next milestone)
- **Directory Structure:** Organization-based directory layout

### 📋 Remaining (Phase 1)
- **Intelligent Sync Engine:** Safe pull vs fetch-only logic
- **Conflict Detection:** Working directory state analysis
- **Daemon Infrastructure:** Background service mode

## Quick Start

```bash
# Install dependencies and build
cargo build

# Initialize configuration and authenticate
cargo run -- init

# Test system health
cargo run -- doctor

# List discoverable repositories
cargo run -- list --details

# Test repository discovery with filtering
cargo run -- list --org MKSG-MugunthKumar
```

## Core Commands

| Command | Description | Status |
|---------|-------------|---------|
| `init` | Setup configuration and authentication | ✅ Working |
| `auth setup/test/status` | Authentication management | ✅ Working |
| `list [--details] [--org ORG]` | Repository discovery and filtering | ✅ Working |
| `sync [--dry-run] [--org ORG]` | Repository synchronization | 🚧 In Progress |
| `daemon start/stop/status` | Background service control | 📋 Planned |
| `doctor` | System diagnostics | ✅ Working |

## Configuration

RepoSentry uses XDG-compliant configuration at `~/.config/reposentry/config.yml`:

```yaml
base_directory: "${HOME}/dev"

filters:
  age:
    max_age: "3month"  # 1month, 3month, 6month
  size:
    max_size: "1GB"    # 100MB, 1GB

github:
  auth_method: "auto"  # auto, gh_cli, token
  include_organizations: true
  exclude_patterns:
    - "archived-*"
    - "test-*"
```

## Technology Stack

- **Language:** Rust
- **Async Runtime:** Tokio
- **GitHub API:** octocrab
- **CLI Framework:** clap
- **Configuration:** serde + TOML
- **Logging:** tracing

---

*For detailed requirements and architecture, see [PRD.md](./PRD.md)*