use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod config;
mod github;

use config::Config;
use github::GitHubClient;

#[derive(Parser)]
#[command(name = "reposentry")]
#[command(about = "Intelligent git repository synchronization daemon")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

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

    // Initialize logging
    init_logging(cli.verbose)?;

    info!("Starting RepoSentry v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = load_config(cli.config).await?;

    // Execute command
    match cli.command {
        Commands::Init { base_dir, skip_auth } => {
            cmd_init(base_dir, skip_auth, &config).await
        }
        Commands::Auth { auth_command } => {
            cmd_auth(auth_command, &config).await
        }
        Commands::Sync { dry_run, force, org } => {
            cmd_sync(dry_run, force, org, &config).await
        }
        Commands::List { details, org } => {
            cmd_list(details, org, &config).await
        }
        Commands::Daemon { daemon_command } => {
            cmd_daemon(daemon_command, &config).await
        }
        Commands::Doctor { component } => {
            cmd_doctor(component, &config).await
        }
    }
}

/// Initialize logging based on verbosity level
fn init_logging(verbose: bool) -> Result<()> {
    let filter = if verbose {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("debug"))
    } else {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info"))
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
        github::auth_setup::setup_authentication().await?;
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
        AuthCommands::Setup => {
            github::auth_setup::setup_authentication().await
        }
        AuthCommands::Test => {
            github::auth_setup::test_authentication(config).await
        }
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
async fn cmd_sync(dry_run: bool, _force: bool, org_filter: Option<String>, config: &Config) -> Result<()> {
    info!("Starting repository synchronization...");

    if dry_run {
        println!("🔍 Dry run mode - no changes will be made");
    }

    // Create GitHub client
    let github_client = GitHubClient::new(config).await?;

    // Get repositories to sync
    let repositories = github_client.get_all_repositories(config).await?;

    // Filter by organization if specified
    let filtered_repos: Vec<_> = if let Some(org) = org_filter {
        repositories.into_iter()
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

    if filtered_repos.is_empty() {
        println!("No repositories found matching criteria");
        return Ok(());
    }

    println!("Found {} repositories to sync:", filtered_repos.len());

    for repo in &filtered_repos {
        println!("  📁 {}", repo.full_name.as_ref().unwrap_or(&repo.name));
    }

    if !dry_run {
        println!("🚧 Synchronization implementation coming soon...");
        // TODO: Implement actual sync logic
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
        repositories.into_iter()
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
async fn cmd_daemon(_daemon_command: DaemonCommands, _config: &Config) -> Result<()> {
    println!("🚧 Daemon functionality coming soon...");
    // TODO: Implement daemon commands
    Ok(())
}

/// System health check and diagnostics
async fn cmd_doctor(_component: Option<DoctorComponent>, config: &Config) -> Result<()> {
    println!("🔍 RepoSentry System Diagnostics");
    println!();

    // Check git installation
    println!("Git Installation:");
    match std::process::Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("  ✅ Git installed: {}", version.trim());
        }
        Ok(_) => {
            println!("  ❌ Git command failed");
        }
        Err(_) => {
            println!("  ❌ Git not found in PATH");
        }
    }

    // Check GitHub authentication
    println!();
    println!("GitHub Authentication:");
    match GitHubClient::new(config).await {
        Ok(client) => {
            println!("  ✅ Authentication successful");
            println!("  👤 Username: {}", client.username());
        }
        Err(e) => {
            println!("  ❌ Authentication failed: {}", e);
        }
    }

    // Check base directory
    println!();
    println!("Base Directory:");
    let expanded_dir = shellexpand::full(&config.base_directory)?;
    if std::path::Path::new(expanded_dir.as_ref()).exists() {
        println!("  ✅ Base directory exists: {}", expanded_dir);
    } else {
        println!("  ⚠️  Base directory does not exist: {}", expanded_dir);
        println!("     Run 'mkdir -p {}' to create it", expanded_dir);
    }

    // Check SSH keys
    println!();
    println!("SSH Configuration:");
    let ssh_dir = dirs::home_dir().unwrap_or_default().join(".ssh");
    if ssh_dir.exists() {
        let ssh_keys = ["id_rsa", "id_ed25519", "id_ecdsa"];
        let found_keys: Vec<_> = ssh_keys.iter()
            .filter(|key| ssh_dir.join(key).exists())
            .collect();

        if found_keys.is_empty() {
            println!("  ⚠️  No SSH keys found in ~/.ssh/");
            println!("     Generate one with: ssh-keygen -t ed25519 -C \"your_email@example.com\"");
        } else {
            println!("  ✅ SSH keys found: {:?}", found_keys);
        }
    } else {
        println!("  ❌ ~/.ssh directory not found");
    }

    println!();
    println!("✅ Diagnostics complete");

    Ok(())
}
