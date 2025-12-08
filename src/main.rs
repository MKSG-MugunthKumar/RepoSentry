use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use reposentry::daemon::is_daemon_running;
use reposentry::github::auth_setup;
use reposentry::tui;
use reposentry::{Config, Daemon, Discovery, GitHubClient, GitHubDiscovery, HealthCheck, SyncEngine};

#[derive(Parser)]
#[command(name = "reposentry")]
#[command(about = "Intelligent git repository synchronization daemon")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Configuration file path (defaults to XDG config location)
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize configuration and authenticate with GitHub
    Init {
        /// Base directory for repositories
        #[arg(short, long, default_value = "~/dev")]
        base_dir: String,

        /// Skip authentication setup
        #[arg(long)]
        skip_auth: bool,
    },

    /// Manage authentication
    Auth {
        #[command(subcommand)]
        auth_command: AuthCommands,
    },

    /// Sync repositories according to configuration
    Sync {
        /// Perform dry run without making changes
        #[arg(long)]
        dry_run: bool,

        /// Force sync even if conflicts detected
        #[arg(long)]
        force: bool,

        /// Sync only specific organization
        #[arg(long)]
        org: Option<String>,
    },

    /// List repositories that would be synced
    List {
        /// Show repository details
        #[arg(long)]
        details: bool,

        /// Filter by organization
        #[arg(long)]
        org: Option<String>,
    },

    /// Run as daemon
    Daemon {
        #[command(subcommand)]
        daemon_command: DaemonCommands,
    },

    /// System health check and diagnostics
    Doctor {
        /// Check specific component
        #[arg(value_enum)]
        component: Option<DoctorComponent>,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Set up authentication
    Setup,

    /// Test current authentication
    Test,

    /// Show authentication status
    Status,
}

#[derive(Subcommand)]
enum DaemonCommands {
    /// Start daemon in foreground
    Start {
        /// Run in foreground (don't daemonize)
        #[arg(long)]
        foreground: bool,
    },

    /// Stop running daemon
    Stop,

    /// Show daemon status
    Status,

    /// Restart daemon
    Restart,

    /// Reload daemon configuration
    Reload,
}

#[derive(clap::ValueEnum, Clone)]
enum DoctorComponent {
    /// Check git installation and configuration
    Git,

    /// Check SSH setup
    Ssh,

    /// Check GitHub authentication
    Auth,

    /// Check filesystem permissions
    Filesystem,

    /// Check all components
    All,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Only initialize logging for CLI commands, not TUI
    // TUI has its own log viewer and stdout logging breaks raw mode
    let is_tui = cli.command.is_none();
    if !is_tui {
        init_logging(cli.verbose)?;
        info!("Starting RepoSentry v{}", env!("CARGO_PKG_VERSION"));
    }

    // Load configuration
    let config = load_config(cli.config).await?;

    // Execute command (default to TUI if no command specified)
    match cli.command {
        None => cmd_tui(&config).await,
        Some(Commands::Init {
            base_dir,
            skip_auth,
        }) => cmd_init(base_dir, skip_auth, &config).await,
        Some(Commands::Auth { auth_command }) => cmd_auth(auth_command, &config).await,
        Some(Commands::Sync {
            dry_run,
            force,
            org,
        }) => cmd_sync(dry_run, force, org, &config).await,
        Some(Commands::List { details, org }) => cmd_list(details, org, &config).await,
        Some(Commands::Daemon { daemon_command }) => cmd_daemon(daemon_command, &config).await,
        Some(Commands::Doctor { component }) => cmd_doctor(component, &config).await,
    }
}

/// Initialize logging based on verbosity level
fn init_logging(verbose: bool) -> Result<()> {
    let filter = if verbose {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"))
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    Ok(())
}

/// Load configuration from specified path or default location
async fn load_config(config_path: Option<std::path::PathBuf>) -> Result<Config> {
    match config_path {
        Some(path) => Config::load(&path),
        None => Config::load_or_default(),
    }
}

/// Initialize RepoSentry configuration and authentication
async fn cmd_init(base_dir: String, skip_auth: bool, config: &Config) -> Result<()> {
    info!("Initializing RepoSentry...");

    // Create directory structure
    let expanded_base_dir = shellexpand::full(&base_dir)?;
    std::fs::create_dir_all(expanded_base_dir.as_ref())?;

    info!("Base directory set to: {}", expanded_base_dir);

    // Update config with new base directory if different
    let mut new_config = config.clone();
    new_config.base_directory = base_dir.clone();

    // Save updated config
    let config_path = Config::default_config_path()?;
    new_config.save(&config_path)?;

    info!("Configuration saved to: {:?}", config_path);

    if !skip_auth {
        info!("Setting up authentication...");
        auth_setup::setup_authentication().await?;
    }

    println!("✅ RepoSentry initialized successfully!");
    println!("   Config: {:?}", config_path);
    println!("   Base directory: {}", expanded_base_dir);

    if !skip_auth {
        println!("   Next: Authenticate with GitHub and run 'reposentry sync'");
    }

    Ok(())
}

/// Handle authentication commands
async fn cmd_auth(auth_command: AuthCommands, config: &Config) -> Result<()> {
    match auth_command {
        AuthCommands::Setup => auth_setup::setup_authentication().await,
        AuthCommands::Test => auth_setup::test_authentication(config).await,
        AuthCommands::Status => {
            match GitHubClient::new(config).await {
                Ok(client) => {
                    println!("✅ Authentication successful");
                    println!("   Username: {}", client.username());
                }
                Err(e) => {
                    println!("❌ Authentication failed: {}", e);
                }
            }
            Ok(())
        }
    }
}

/// Sync repositories according to configuration
async fn cmd_sync(
    dry_run: bool,
    force: bool,
    org_filter: Option<String>,
    config: &Config,
) -> Result<()> {
    info!("Starting repository synchronization...");

    // Create discovery and sync engine
    let discovery = GitHubDiscovery::new(config.clone()).await?;
    let sync_engine = SyncEngine::new(config.clone());

    // Discover repositories
    println!("🔍 Discovering repositories...");
    let repos = discovery.discover().await?;
    println!("   Found {} repositories", repos.len());

    if dry_run {
        println!("\n🔍 Dry run mode - analyzing repository states");

        let repo_states = sync_engine.analyze_repos(&repos).await?;

        println!("📊 Repository Analysis Results:");

        let mut needs_clone = 0;
        let mut needs_pull = 0;
        let mut has_conflicts = 0;
        let mut up_to_date = 0;

        for state in &repo_states {
            match (
                state.exists,
                state.has_uncommitted_changes,
                state.is_ahead_of_remote,
                state.is_behind_remote,
            ) {
                (false, _, _, _) => {
                    needs_clone += 1;
                    println!("   📥 Clone needed: {}", state.path.display());
                }
                (true, true, _, _) => {
                    has_conflicts += 1;
                    println!(
                        "   ⚠️  Has conflicts: {} (uncommitted changes)",
                        state.path.display()
                    );
                }
                (true, false, _, true) => {
                    needs_pull += 1;
                    println!(
                        "   🔄 Pull needed: {} (behind remote)",
                        state.path.display()
                    );
                }
                _ => {
                    up_to_date += 1;
                    if repo_states.len() <= 10 {
                        // Show details for small sets
                        println!("   ✅ Up to date: {}", state.path.display());
                    }
                }
            }
        }

        println!("\n📈 Summary:");
        println!("   📥 Repositories to clone: {}", needs_clone);
        println!("   🔄 Repositories to pull: {}", needs_pull);
        println!("   ⚠️  Repositories with conflicts: {}", has_conflicts);
        println!("   ✅ Up-to-date repositories: {}", up_to_date);

        if has_conflicts > 0 {
            println!("\n💡 Tip: Resolve conflicts manually before running sync");
        }

        return Ok(());
    }

    // Real sync mode
    println!("\n🔄 Running full repository synchronization");

    if force {
        println!("⚡ Force mode enabled");
    }

    let summary = sync_engine.sync_repos(repos).await?;

    println!("\n🎉 Synchronization Complete!");
    println!("   📊 Total repositories: {}", summary.total_repositories);
    println!(
        "   ✅ Successful operations: {}",
        summary.successful_operations
    );
    println!("   ❌ Failed operations: {}", summary.failed_operations);
    println!("   ⏭️  Skipped operations: {}", summary.skipped_operations);
    println!("   ⏱️  Duration: {:.2}s", summary.duration.as_secs_f64());

    if summary.failed_operations > 0 {
        println!("\n🔍 Failed Operations:");
        for result in &summary.results {
            if let reposentry::SyncResult::Failed { path, error } = result {
                println!("   ❌ {}: {}", path.display(), error);
            }
        }
    }

    if let Some(org) = org_filter {
        println!("\n📝 Note: Filtered by organization: {}", org);
        println!("   Use --help to see all filtering options");
    }

    Ok(())
}

/// List repositories that would be synced
async fn cmd_list(details: bool, org_filter: Option<String>, config: &Config) -> Result<()> {
    info!("Listing repositories...");

    // Create GitHub client
    let github_client = GitHubClient::new(config).await?;

    // Get repositories
    let repositories = github_client.get_all_repositories(config).await?;

    // Filter by organization if specified
    let filtered_repos: Vec<_> = if let Some(org) = org_filter {
        repositories
            .into_iter()
            .filter(|repo| {
                repo.full_name
                    .as_ref()
                    .map(|name| name.starts_with(&format!("{}/", org)))
                    .unwrap_or(false)
            })
            .collect()
    } else {
        repositories
    };

    println!("Repositories ({}): ", filtered_repos.len());

    for repo in filtered_repos {
        if details {
            println!("📁 {}", repo.full_name.as_ref().unwrap_or(&repo.name));
            if let Some(description) = &repo.description {
                println!("   📝 {}", description);
            }
            if let Some(size) = repo.size {
                println!("   📊 Size: {} KB", size);
            }
            if let Some(updated) = repo.updated_at {
                println!("   🕒 Updated: {}", updated.format("%Y-%m-%d"));
            }
            if let Some(url) = &repo.html_url {
                println!("   🔗 {}", url);
            }
            println!();
        } else {
            println!("  📁 {}", repo.full_name.as_ref().unwrap_or(&repo.name));
        }
    }

    Ok(())
}

/// Handle daemon commands
async fn cmd_daemon(daemon_command: DaemonCommands, config: &Config) -> Result<()> {
    match daemon_command {
        DaemonCommands::Start { foreground } => {
            println!("🚀 Starting RepoSentry daemon...");

            // Check if daemon is already running
            if is_daemon_running(config)? {
                println!("⚠️  Daemon is already running!");
                println!("   Use 'reposentry daemon stop' to stop it first");
                return Ok(());
            }

            let mut daemon = Daemon::new((*config).clone()).await?;

            if foreground {
                println!("🖥️  Running in foreground mode (Ctrl+C to stop)");
                daemon.run().await?;
            } else {
                #[cfg(unix)]
                {
                    daemon.daemonize()?;
                    println!("✅ Daemon started in background");
                    println!("   PID file: {}", config.daemon.pid_file);
                    println!("   Log file: {}", config.daemon.log_file);
                    println!("   Sync interval: {}", config.daemon.interval);
                }

                #[cfg(not(unix))]
                {
                    println!("❌ Background daemon mode not supported on this platform");
                    println!("   Use --foreground to run in foreground mode");
                    return Ok(());
                }
            }
        }

        DaemonCommands::Stop => {
            println!("🛑 Stopping RepoSentry daemon...");

            if !is_daemon_running(config)? {
                println!("⚠️  No daemon appears to be running");
                return Ok(());
            }

            let daemon = Daemon::new((*config).clone()).await?;
            daemon.stop().await?;

            println!("✅ Daemon stop signal sent");
        }

        DaemonCommands::Status => {
            println!("📊 RepoSentry Daemon Status");

            let is_running = is_daemon_running(config)?;

            if is_running {
                let daemon = Daemon::new((*config).clone()).await?;
                let status = daemon.status(std::time::Instant::now());

                println!("   🟢 Status: Running");
                println!("   ⏱️  Uptime: {:.1}m", status.uptime.as_secs_f64() / 60.0);
                println!("   🔄 Sync interval: {}", config.daemon.interval);

                if let Some(next_sync) = status.next_sync_in {
                    println!("   ⏰ Next sync in: {:.0}s", next_sync.as_secs_f64());
                }

                println!("   📊 Sync statistics:");
                println!("      Total: {}", status.total_syncs);
                println!("      Successful: {}", status.successful_syncs);
                println!("      Failed: {}", status.failed_syncs);

                if !config.daemon.log_file.is_empty() {
                    println!("   📄 Log file: {}", config.daemon.log_file);
                }
            } else {
                println!("   🔴 Status: Not running");
                println!("   💡 Use 'reposentry daemon start' to start the daemon");
            }
        }

        DaemonCommands::Restart => {
            println!("🔄 Restarting RepoSentry daemon...");

            if is_daemon_running(config)? {
                println!("🛑 Stopping current daemon...");
                let daemon = Daemon::new((*config).clone()).await?;
                daemon.stop().await?;

                // Wait a moment for clean shutdown
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }

            println!("🚀 Starting daemon...");
            let daemon = Daemon::new((*config).clone()).await?;

            #[cfg(unix)]
            {
                daemon.daemonize()?;
                println!("✅ Daemon restarted in background");
            }

            #[cfg(not(unix))]
            {
                println!("❌ Background daemon mode not supported on this platform");
                println!("   Use 'daemon start --foreground' to run in foreground mode");
            }
        }

        DaemonCommands::Reload => {
            println!("🔄 Reloading daemon configuration...");

            if !is_daemon_running(config)? {
                println!("⚠️  No daemon appears to be running");
                return Ok(());
            }

            // TODO: Implement configuration hot-reload via IPC
            println!("🚧 Configuration hot-reload coming soon...");
            println!("   For now, use 'daemon restart' to apply new configuration");
        }
    }

    Ok(())
}

/// System health check and diagnostics
async fn cmd_doctor(_component: Option<DoctorComponent>, config: &Config) -> Result<()> {
    let health = HealthCheck::run(config).await;
    print_health_report(&health);
    Ok(())
}

/// Launch the Terminal User Interface
async fn cmd_tui(config: &Config) -> Result<()> {
    // Preflight checks - ensure system is properly configured
    let health = HealthCheck::run(config).await;

    if !health.all_passed() {
        print_health_report(&health);
        println!();
        println!("❌ Cannot start TUI - fix the errors above first");
        std::process::exit(1);
    }

    // Show warnings but continue
    let warnings = health.warnings();
    if !warnings.is_empty() {
        for warning in warnings {
            println!("⚠️  {}", warning.message);
            if let Some(details) = &warning.details {
                println!("   {}", details);
            }
        }
        println!();
    }

    // Clone config for TUI ownership
    let config = config.clone();

    // Run the TUI
    tui::run_tui(config).await?;

    Ok(())
}

/// Print health check report to stdout
fn print_health_report(health: &HealthCheck) {
    use reposentry::health::CheckResult;

    fn print_check(name: &str, result: &CheckResult) {
        println!("{}:", name);
        let icon = if result.passed {
            if result.is_warning { "⚠️ " } else { "✅" }
        } else {
            "❌"
        };
        println!("  {} {}", icon, result.message);
        if let Some(details) = &result.details {
            for line in details.lines() {
                println!("     {}", line);
            }
        }
    }

    println!("🔍 RepoSentry System Diagnostics");
    println!();

    for (name, result) in health.all_checks() {
        print_check(name, result);
        println!();
    }

    if health.all_passed() {
        println!("✅ All checks passed");
    } else {
        println!("❌ Some checks failed");
    }
}
