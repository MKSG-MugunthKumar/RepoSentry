# RepoSentry - Product Requirements Document

**Version:** 1.0
**Date:** November 2025
**Author:** MK

## Executive Summary

RepoSentry is an intelligent git repository synchronization daemon that automatically keeps local repository collections in sync with remote origins. Unlike traditional git tools that only fetch, RepoSentry intelligently pulls changes when safe or falls back to fetch-only when conflicts are detected, ensuring repository integrity while maximizing automation.

## Product Vision

**Mission:** Eliminate manual repository maintenance overhead for developers managing multiple repositories while preventing data loss from careless automated pulls.

**Vision:** The definitive solution for intelligent, background repository synchronization across any git hosting platform.

## Problem Statement

### Current Pain Points

1. **Manual Overhead:** Developers must manually pull updates across dozens of repositories
2. **Git Limitations:** Native `git` only provides auto-fetch, not intelligent auto-pull
3. **Conflict Risk:** Automated pulling can overwrite local changes or create merge conflicts
4. **Platform Lock-in:** Existing tools are tied to specific hosting providers (GitHub, GitLab)
5. **No Intelligence:** Current tools don't assess safety before pulling changes

### Target Users

- **Primary:** Software developers managing 10+ repositories
- **Secondary:** DevOps engineers maintaining organization codebases
- **Tertiary:** Technical teams using multi-repository architectures

## Product Goals

### Primary Goals
- ✅ **Safety First:** Never lose local changes through intelligent conflict detection
- ✅ **Background Operation:** Run as daemon with minimal user intervention
- ✅ **Platform Agnostic:** Support any git hosting solution (GitHub, GitLab, self-hosted)
- ✅ **Git Native:** Use standard git commands for all operations (no proprietary APIs for git ops)

### Secondary Goals
- ⭐ **Performance:** Parallel processing for faster synchronization
- ⭐ **Observability:** Rich logging and metrics for monitoring
- ⭐ **Configuration:** Flexible filtering and customization options

## Core Features

### MVP (Version 1.0)

#### Intelligent Sync Engine
- **Safe Auto-Pull:** Pull changes only when no conflicts detected
- **Fallback to Fetch:** Automatically fetch-only when conflicts exist
- **Working Directory Protection:** Never modify uncommitted local changes
- **Branch Awareness:** Respect current branch and only sync appropriate changes

#### Repository Discovery
- **GitHub Integration:** Use GitHub API via `octocrab` for repository enumeration
- **Organization Support:** Scan personal and organization repositories
- **SSH/HTTPS Support:** Respect user's existing git authentication setup
- **Local Directory Management:** Organize repositories in configurable directory structure

#### Daemon Mode
- **Background Service:** Run continuously with configurable sync intervals
- **System Integration:** Proper daemon management (systemd on Linux, launchd on macOS)
- **Resource Efficient:** Low memory and CPU footprint during idle periods
- **Graceful Shutdown:** Clean exit with proper cleanup of ongoing operations

#### Monitoring & Logging
- **Structured Logging:** JSON-formatted logs for easy parsing and monitoring
- **Status Reporting:** Current sync status and repository health
- **Error Handling:** Detailed error reporting with actionable suggestions
- **Metrics Export:** Basic metrics for sync success/failure rates

### Post-MVP Features (Version 1.1+)

#### Enhanced Git Provider Support
- **GitLab Integration:** Full GitLab API support
- **Self-hosted Git:** Generic git server support via SSH key scanning
- **Gitea/Forgejo:** Support for lightweight self-hosted solutions
- **Multi-provider:** Manage repositories across different hosting providers simultaneously

#### Advanced Sync Intelligence
- **Conflict Resolution:** Interactive conflict resolution with user prompts
- **Stash Integration:** Automatically stash and reapply local changes when safe
- **Merge Strategy Options:** Configurable merge strategies (fast-forward only, rebase, merge)
- **Dirty Repository Handling:** Smart handling of repositories with uncommitted changes

#### Performance & Scale
- **Parallel Operations:** Concurrent repository processing with rate limiting
- **Delta Sync:** Only process repositories with detected changes
- **Bandwidth Optimization:** Shallow clones and efficient fetch strategies
- **Caching Layer:** Repository metadata caching to reduce API calls

#### User Experience
- **Interactive TUI:** Terminal UI for repository selection and status monitoring
- **CLI Commands:** Rich command-line interface for manual operations
- **Configuration Management:** TOML/YAML configuration files
- **Shell Integration:** Command completions for bash/zsh/fish

#### Developer Features
- **Repository Analytics:** Size, language, activity metrics
- **Dependency Scanning:** Security vulnerability detection
- **Custom Filters:** Filter repositories by language, size, activity, labels
- **Webhook Integration:** React to repository events in real-time

## Technical Architecture

### Technology Stack
- **Language:** Rust (performance, safety, cross-platform)
- **Async Runtime:** Tokio for efficient I/O and concurrency
- **GitHub API:** octocrab crate for GitHub integration
- **Configuration:** serde with TOML/YAML support
- **CLI Framework:** clap for argument parsing and commands
- **Logging:** tracing for structured logging
- **Terminal UI:** ratatui for interactive interface

### Core Components

#### Sync Engine (`src/sync/`)
```rust
pub struct SyncEngine {
    config: Config,
    git_client: GitClient,
    api_clients: HashMap<Provider, Box<dyn ApiClient>>,
}
```

#### Git Operations (`src/git/`)
- **GitClient:** Wrapper around git CLI operations
- **ConflictDetector:** Analyze repositories for potential conflicts
- **SafePuller:** Intelligent pull logic with fallback strategies

#### API Clients (`src/api/`)
- **GitHub:** octocrab-based GitHub API client
- **Trait-based:** Common interface for all git hosting providers
- **Rate Limiting:** Built-in rate limiting and retry logic

#### Daemon (`src/daemon/`)
- **Service Management:** Platform-specific daemon lifecycle
- **Scheduler:** Configurable sync intervals and triggers
- **Health Monitoring:** Service health checks and recovery

### Configuration Schema

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
strategy = "safe-pull" # safe-pull, fetch-only, interactive
auto_stash = false
ff_only = true
conflict_resolution = "prompt" # prompt, skip, fetch-only

[logging]
level = "info"
format = "json"
file = "/var/log/reposentry.log"
```

## Success Metrics

### Key Performance Indicators (KPIs)

#### User Adoption
- **Target:** 1,000+ active installations within 6 months
- **Metric:** Monthly active daemon instances

#### Reliability
- **Target:** 99.9% uptime for daemon operations
- **Metric:** Successful sync operations / total sync attempts

#### Safety
- **Target:** Zero data loss incidents
- **Metric:** Conflicts detected and safely handled vs. merge conflicts created

#### Performance
- **Target:** <5 minute full sync for 100 repositories
- **Metric:** Average time to complete full repository sync

### User Experience Metrics
- **Setup Time:** <2 minutes from install to first successful sync
- **Error Recovery:** 95% of errors provide actionable resolution steps
- **Resource Usage:** <50MB memory usage during idle periods

## Risk Assessment

### Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|---------|------------|
| Git authentication complexity | High | Medium | Delegate to existing git config, extensive documentation |
| API rate limiting | Medium | Medium | Intelligent caching, exponential backoff |
| Cross-platform compatibility | Medium | High | Comprehensive CI/CD testing across platforms |
| Data loss from automated pulls | Low | Critical | Extensive testing, conservative conflict detection |

### Market Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|---------|------------|
| Competing solutions emerge | Medium | Medium | Focus on unique safety features and git-native approach |
| Changes to git hosting APIs | Low | High | Provider abstraction layer, multiple provider support |
| User adoption challenges | Medium | High | Excellent documentation, gradual feature rollout |

## Competitive Analysis

### Existing Solutions

#### Git's Built-in Tools
- **Strengths:** Universal compatibility, no additional dependencies
- **Weaknesses:** Fetch-only, no intelligence, manual operation required
- **Differentiation:** RepoSentry adds intelligence and automation

#### GitHub CLI (`gh`)
- **Strengths:** Official GitHub support, rich feature set
- **Weaknesses:** GitHub-only, no daemon mode, no intelligent sync
- **Differentiation:** Multi-provider support, background operation, safety

#### Custom Shell Scripts
- **Strengths:** Customizable, simple
- **Weaknesses:** Brittle, platform-specific, no intelligence
- **Differentiation:** Robust error handling, cross-platform, intelligent conflict detection

### Unique Value Proposition

RepoSentry is the **only** solution that combines:
1. **Intelligent Safety:** Never overwrites local changes
2. **Git-Native Operations:** Uses standard git commands, respects existing authentication
3. **Multi-Provider Support:** Not locked to specific hosting providers
4. **Daemon Architecture:** True background operation
5. **Developer-First Design:** Built by developers, for developers

## Development Roadmap

### Phase 1: MVP (Months 1-3) - 85% Complete
- ✅ **GitHub API Integration:** Complete octocrab implementation with authentication auto-detection
- ✅ **CLI Interface:** Full command structure implemented (init, auth, list, sync, daemon, doctor)
- ✅ **Configuration System:** XDG-compliant YAML with age/size filtering and pattern exclusions
- ✅ **Repository Discovery:** User and organization repository enumeration with filtering
- ✅ **Authentication:** Auto-detection via GitHub CLI and GITHUB_TOKEN environment variable
- 🚧 **Core Sync Engine:** Git operations framework in progress (cloning, pulling, conflict detection)
- 🚧 **Directory Management:** Organization-based directory structure implementation
- 📋 **Intelligent Pull Logic:** Safe pull vs fetch-only decision engine (remaining)
- 📋 **Daemon Infrastructure:** Background service mode and scheduling (remaining)
- ✅ **Cross-platform Support:** Rust-based implementation with XDG compliance

### Phase 2: Enhanced Features (Months 4-6)
- ⭐ Interactive TUI for repository management
- ⭐ Advanced configuration options and filtering
- ⭐ Performance optimizations and parallel processing
- ⭐ Comprehensive logging and monitoring
- ⭐ GitLab API support

### Phase 3: Enterprise Features (Months 7-12)
- 🚀 Self-hosted git server support
- 🚀 Advanced conflict resolution
- 🚀 Repository analytics and insights
- 🚀 Webhook integration for real-time updates
- 🚀 Enterprise deployment guides

### Phase 4: Ecosystem (Year 2)
- 🌟 Plugin architecture for extensibility
- 🌟 Integration with development tools (IDEs, CI/CD)
- 🌟 Advanced security features
- 🌟 Team collaboration features

## Launch Strategy

### Open Source Approach
- **Repository:** GitHub public repository under MIT license
- **Community:** Encourage contributions and feedback from day one
- **Documentation:** Comprehensive docs with examples and tutorials

### Distribution
- **Cargo:** Primary distribution through crates.io
- **Package Managers:** Homebrew (macOS), AUR (Arch Linux), apt/yum repos
- **Binary Releases:** GitHub Releases with pre-built binaries
- **Docker:** Container images for easy deployment

### Marketing & Outreach
- **Developer Communities:** Hacker News, Reddit r/rust, r/programming
- **Conferences:** Present at Rust conferences and developer meetups
- **Blog Posts:** Technical deep-dives on intelligent git synchronization
- **Documentation:** Stellar documentation as a competitive advantage

## Success Definition

### Version 1.0 Success Criteria
1. **✅ Functional MVP:** All core features working reliably
2. **✅ Cross-Platform:** Tested and working on Linux, macOS, Windows
3. **✅ Documentation:** Complete user and developer documentation
4. **✅ Zero Data Loss:** No reported incidents of local changes being overwritten
5. **✅ Community Adoption:** 100+ GitHub stars within first month

### Long-term Success Vision
- **Industry Standard:** RepoSentry becomes the de facto tool for repository synchronization
- **Community Growth:** Active contributor community with regular contributions
- **Enterprise Adoption:** Used by development teams at major technology companies
- **Platform Integration:** Integrated into popular development tools and IDEs

## Implementation Notes (Phase 1)

### ✅ Completed Components

**GitHub Integration:**
- Authentication strategy auto-detection with fallback hierarchy (gh CLI → GITHUB_TOKEN → error with setup guidance)
- Complete repository discovery supporting both user and organization repositories
- Comprehensive filtering: age-based (1/3/6 months), size-based (100MB/1GB), pattern exclusions with glob matching
- Proper pagination handling for large repository collections (255 page limit with u8 constraints)

**CLI Architecture:**
- Structured command interface with clap derive macros for type safety
- Comprehensive subcommands: `init`, `auth [setup|test|status]`, `list`, `sync`, `daemon`, `doctor`
- Integrated help system and verbose logging support
- XDG Base Directory Specification compliance for configuration placement

**Configuration System:**
- Type-safe YAML configuration with serde
- Environment variable expansion for paths (${HOME}, ${XDG_DATA_HOME}, etc.)
- Graceful fallback handling for missing XDG variables
- Default configuration creation on first run

**System Diagnostics:**
- Comprehensive `doctor` command checking git installation, SSH keys, authentication, and filesystem permissions
- Clear status reporting with actionable error messages
- Integration health verification

### 🚧 Current Development Focus

**Git Operations Module:**
- Repository cloning with proper remote setup
- Organization-based directory structure (`~/dev/org/repo` vs `~/dev/repo`)
- Parallel processing with tokio for concurrent operations
- Error handling and cleanup for failed operations

**Intelligent Sync Logic:**
- Working directory state detection
- Conflict detection before pulling
- Safe pull vs fetch-only decision engine
- Timestamp preservation from git commit history

### 📋 Implementation Lessons Learned

1. **octocrab API Evolution:** Updated from v0.38 to v0.48 required API method changes (`list_repos_for_authenticated_user` vs direct repo access)
2. **XDG Compliance:** Environment variable handling requires graceful fallbacks for missing XDG variables
3. **Type Safety:** Using u8 for pagination limits prevents overflow while matching API constraints
4. **Error UX:** Providing setup guidance in error messages improves user onboarding experience
5. **Configuration Patterns:** Default value functions enable clean serde defaults while supporting environment-specific overrides

---

*This PRD serves as the foundational document for RepoSentry development. It will be updated iteratively as we gather user feedback and validate assumptions through the development process.*