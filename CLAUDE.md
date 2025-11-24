# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**RepoSentry** is an intelligent git repository synchronization daemon currently in the planning/early development phase. It automatically keeps local repository collections in sync with remote origins while intelligently preventing data loss from careless automated pulls.

**Current Status:** Planning phase - no Rust code yet, comprehensive documentation-driven approach

## Development Commands

### Initial Setup (Not Yet Done)
```bash
# Initialize Rust project structure
cargo init

# Add planned dependencies
cargo add tokio --features full
cargo add octocrab
cargo add clap --features derive
cargo add serde --features derive
cargo add toml
cargo add tracing
cargo add tracing-subscriber
```

### Build and Development (Future)
```bash
# Build the project
cargo build

# Run with development features
cargo run

# Run tests
cargo test

# Check code quality
cargo clippy
cargo fmt

# Build for release
cargo build --release
```

### Testing the Reference Implementation
```bash
# Test the working bash reference implementation
./legacy-pull-script.sh

# Make it executable if needed
chmod +x legacy-pull-script.sh
```

## High-Level Architecture

### Planned Project Structure
```
src/
├── sync/                    # Core sync engine
│   ├── engine.rs           # Main synchronization logic
│   └── strategies.rs       # Different sync strategies (safe-pull, fetch-only)
├── git/                    # Git operations wrapper
│   ├── client.rs          # Git CLI operations wrapper
│   └── conflict_detector.rs # Analyze repos for conflicts before pulling
├── api/                    # Git provider clients
│   ├── github.rs          # GitHub API integration via octocrab
│   └── traits.rs          # Common interface for all git hosting providers
├── daemon/                 # Background service
│   ├── service.rs         # Platform-specific daemon lifecycle
│   ├── scheduler.rs       # Configurable sync intervals
│   └── health.rs          # Service health monitoring
├── config/                 # Configuration management
├── cli/                    # Command-line interface
├── logging/                # Structured logging setup
└── main.rs
```

### Core Design Principles

1. **Safety First**: Never lose local changes through conservative conflict detection
2. **Git Native**: Only uses standard git commands, respects existing authentication
3. **Provider Agnostic**: Abstract interface supports any git hosting platform
4. **Cross-Platform**: Designed for Linux, macOS, and Windows
5. **Trait-Based Architecture**: Extensible design for future providers

### Key Components

- **SyncEngine**: Main orchestrator that manages repository discovery and synchronization
- **GitClient**: Wrapper around git CLI operations with intelligent conflict detection
- **ApiClient Trait**: Common interface implemented by GitHub, GitLab, and future providers
- **SafePuller**: Core logic that decides between pull vs fetch-only based on repository state

## Configuration Schema

The planned TOML configuration format:

```toml
[reposentry]
base_directory = "~/dev"
sync_interval = "30m"
max_parallel_repos = 4

[providers.github]
enabled = true
username = "MKSG-MugunthKumar"
include_orgs = true
exclude_repos = ["archived-*", "test-*"]

[sync]
strategy = "safe-pull"      # safe-pull, fetch-only, interactive
auto_stash = false
ff_only = true
conflict_resolution = "prompt"  # prompt, skip, fetch-only

[logging]
level = "info"
format = "json"
file = "/var/log/reposentry.log"
```

## Reference Implementation

The `legacy-pull-script.sh` file contains a working bash implementation that serves as the functional specification for the Rust version. Key features implemented:

- GitHub CLI integration for repository discovery
- Personal and organization repository support
- Intelligent timestamp preservation from git commit history
- Comprehensive error handling and logging
- Safe directory management

**Important**: The bash script demonstrates the exact behavior the Rust implementation should replicate.

## Technology Stack

- **Language**: Rust (chosen for performance, safety, cross-platform support)
- **Async Runtime**: Tokio for efficient I/O operations
- **GitHub API**: octocrab crate for GitHub integration
- **CLI Framework**: clap for argument parsing
- **Configuration**: serde + TOML for configuration management
- **Logging**: tracing for structured logging
- **Future TUI**: ratatui for interactive terminal interface

## Development Workflow

1. **Start with Core**: Implement GitClient wrapper and basic conflict detection
2. **Add Provider Support**: Begin with GitHub API integration using octocrab
3. **Build Sync Engine**: Implement the intelligent pull/fetch logic
4. **Daemon Infrastructure**: Add background service capabilities
5. **CLI Interface**: Create user-facing command interface
6. **Cross-Platform Testing**: Ensure compatibility across target platforms

## Safety Requirements

**Critical**: This tool must never lose user data. Key safety principles:

- Always detect conflicts before attempting to pull
- Fall back to fetch-only when conflicts are detected
- Never modify repositories with uncommitted changes
- Provide clear error messages with actionable resolution steps
- Extensive testing with various repository states before any release

## Next Development Steps

1. Run `cargo init` to initialize the Rust project structure
2. Add core dependencies listed in the PRD
3. Implement basic GitClient wrapper with conflict detection
4. Study the bash reference implementation for exact behavior replication
5. Create GitHub API integration using octocrab
6. Implement the core SyncEngine with safe-pull logic

## Documentation References

- `PRD.md`: Complete product requirements with detailed feature specifications
- `README.md`: Project overview and technology stack summary
- `legacy-pull-script.sh`: Working reference implementation in bash