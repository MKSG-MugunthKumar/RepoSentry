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

YAML configuration format (see `config.example.yml` for full reference):

```yaml
base_directory: "~/dev"

github:
  username: "your-username"
  include_orgs: true

sync:
  strategy: "safe-pull"      # safe-pull, fetch-only, interactive
  auto_stash: false
  fast_forward_only: true
  max_parallel: 4

daemon:
  enabled: false
  interval: "30m"
  pid_file: "reposentry.pid"  # Filename only - placed in XDG_RUNTIME_DIR
  log_file: "daemon.log"      # Filename only - placed in XDG_DATA_HOME/reposentry

logging:
  level: "info"
  format: "compact"
  color: true

advanced:
  preserve_timestamps: true   # Set directory mtime to latest commit date
  verify_clone: false
  cleanup_on_error: true
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

1. ✅ **Foundation**: Rust project structure with comprehensive Cargo.toml
2. ✅ **Configuration**: XDG-compliant YAML configuration with type-safe serde
3. ✅ **Authentication**: GitHub CLI + GITHUB_TOKEN auto-detection (**TESTED AND WORKING**)
4. ✅ **CLI Framework**: Complete command structure with clap (**TESTED AND WORKING**)
5. ✅ **Repository Discovery**: octocrab integration with filtering (**TESTED: 161→33 repos**)
6. 🚧 **Git Operations**: Parallel cloning and sync operations (next milestone)
7. 📋 **Intelligent Sync**: Safe pull vs fetch-only logic
8. 📋 **Daemon Infrastructure**: Background service mode

## Current Implementation Status

**Project Status: Phase 2 Development (85% Complete)**

- **Phase 1 (Foundation & Discovery)**: 95% Complete ✅
- **Phase 2 (Git Operations & Sync)**: 50% Complete 🚧
- **Phase 3 (Daemon & Production)**: 0% Pending 📋

📊 **Detailed Implementation Status**: See [`docs/implementation-status.md`](./docs/implementation-status.md)

### Current Development Focus

🚧 **Active Development (Phase 2)**:
- **Git Operations** (`src/git.rs`): Core framework implemented, testing in progress
- **Sync Engine**: Safe pull strategies and conflict detection
- **CLI Integration**: Connecting new git operations to existing commands

📋 **Next Milestones**:
- Sync command implementation with parallel processing
- Daemon infrastructure for background synchronization
- End-to-end workflow testing and performance optimization

## Safety Requirements

**Critical**: This tool must never lose user data. Key safety principles:

- Always detect conflicts before attempting to pull
- Fall back to fetch-only when conflicts are detected
- Never modify repositories with uncommitted changes
- Provide clear error messages with actionable resolution steps
- Extensive testing with various repository states before any release

## Implementation Best Practices Discovered

### 🔧 Development Patterns

**1. Configuration Management**
- Use default value functions (`fn default_true() -> bool`) for clean serde defaults
- Handle missing environment variables gracefully with fallback logic
- Expand environment variables after loading, not during parsing
- Place configuration in XDG-compliant locations with proper error messages

**2. Error Handling & UX**
- Provide setup guidance in error messages (not just "authentication failed")
- Use structured error chains with anyhow for context preservation
- Include actionable next steps in error messages
- Test error paths with missing dependencies

**3. API Integration**
- Pin dependency versions for stability (octocrab 0.48 vs 0.38)
- Handle API pagination limits properly (u8 constraints vs u32 expectations)
- Implement comprehensive filtering at the API level to reduce data transfer
- Use proper authentication hierarchy with fallbacks

**4. CLI Design**
- Use clap derive macros for type-safe argument parsing
- Structure commands hierarchically (auth [setup|test|status])
- Include both short and long help descriptions
- Implement verbose logging with environment variable support

**5. Async Architecture**
- Use tokio for all async operations with proper feature flags
- Implement proper pagination with async iterators
- Handle concurrent operations with proper error propagation
- Use appropriate timeout values for network operations

**6. Testing & Validation Best Practices (Updated 2025-11-24)**
- Always test functionality with timeout commands to avoid broken pipe issues
- Use actual repository counts to validate filtering effectiveness
- Test authentication strategies in isolation before integration testing
- Monitor API rate limits during development with debug logging
- Validate environment variable expansion in configuration loading
- Test CLI help system completeness for all subcommands

**7. Performance Optimization**
- Repository discovery scales well: ~8 seconds for 161 repositories across 5 data sources
- Filtering is highly effective: 79% reduction (161→33) with moderate 3month/1GB limits
- GitHub CLI authentication is faster than token-based authentication
- Logging level should be configurable to reduce output noise during normal operation

### 🔍 Testing Strategies

**System Integration Testing**
```bash
# Test full system health
cargo run -- doctor

# Test authentication without side effects
cargo run -- auth test

# Test repository discovery with filtering
cargo run -- list --org MKSG-MugunthKumar

# Test configuration generation
rm ~/.config/reposentry/config.yml
cargo run -- init --skip-auth
```

**New Performance Testing Insights (2025-11-24)**
```bash
# Test repository discovery performance (~8 seconds for 161 repos)
time cargo run -- list --details

# Test filtering effectiveness (161 → 33 repos with 3month/1GB)
cargo run -- list --details | grep "Total repositories\|Repositories after filtering"

# Test broken pipe handling with timeout
timeout 10s cargo run -- list --details  # Avoids broken pipe errors

# Test organization discovery
cargo run -- list --org specific-org-name
```

**Development Workflow Testing**
```bash
# Test compilation
cargo check

# Test with different log levels
RUST_LOG=debug cargo run -- list

# Test error handling
GITHUB_TOKEN="" cargo run -- auth status
```

## Next Development Steps

1. ✅ Complete foundation (Rust project, config, CLI, GitHub integration)
2. 🚧 Implement `src/git.rs` for repository operations
3. 📋 Add organization-based directory structure logic
4. 📋 Implement intelligent sync engine with conflict detection
5. 📋 Add daemon infrastructure with background scheduling
6. 📋 Complete comprehensive testing across platforms

## Documentation References

- `PRD.md`: Complete product requirements with detailed feature specifications
- `README.md`: Project overview and technology stack summary
- `legacy-pull-script.sh`: Working reference implementation in bash