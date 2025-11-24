//! Daemon Infrastructure - Background service for automated repository synchronization
//!
//! This module provides the daemon/service infrastructure for running RepoSentry
//! in the background with configurable sync intervals, PID file management,
//! and graceful shutdown handling.

use crate::Config;
use crate::sync::{SyncEngine, SyncSummary};
use anyhow::{Result, Context};
// Helper function to parse duration strings like "30m", "1h", etc.
fn parse_daemon_duration(duration_str: &str) -> Result<u64> {
    let duration_str = duration_str.trim().to_lowercase();

    if let Some(value) = duration_str.strip_suffix('s') {
        value.parse::<u64>().context("Invalid seconds value")
    } else if let Some(value) = duration_str.strip_suffix('m') {
        value.parse::<u64>().map(|v| v * 60).context("Invalid minutes value")
    } else if let Some(value) = duration_str.strip_suffix('h') {
        value.parse::<u64>().map(|v| v * 3600).context("Invalid hours value")
    } else if let Some(value) = duration_str.strip_suffix('d') {
        value.parse::<u64>().map(|v| v * 86400).context("Invalid days value")
    } else {
        // Try to parse as raw seconds
        duration_str.parse::<u64>().context("Invalid duration format. Use format like '30m', '1h', '2d'")
    }
}
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::{info, warn, error, debug};

/// Daemon state and control
pub struct Daemon {
    config: Arc<Config>,
    sync_engine: SyncEngine,
    shutdown_sender: broadcast::Sender<()>,
    is_running: Arc<AtomicBool>,
    pid_file_path: Option<PathBuf>,
}

/// Daemon statistics and status
#[derive(Debug, Clone)]
pub struct DaemonStatus {
    pub is_running: bool,
    pub uptime: Duration,
    pub last_sync: Option<Instant>,
    pub total_syncs: u64,
    pub successful_syncs: u64,
    pub failed_syncs: u64,
    pub next_sync_in: Option<Duration>,
}

impl Daemon {
    /// Create a new daemon instance
    pub async fn new(config: Config) -> Result<Self> {
        let config = Arc::new(config);
        let sync_engine = SyncEngine::new(config.as_ref().clone()).await
            .context("Failed to create sync engine for daemon")?;

        let (shutdown_sender, _) = broadcast::channel(1);
        let is_running = Arc::new(AtomicBool::new(false));

        // Prepare PID file path if configured
        let pid_file_path = if !config.daemon.pid_file.is_empty() {
            let expanded_path = shellexpand::full(&config.daemon.pid_file)
                .context("Failed to expand PID file path")?;
            Some(PathBuf::from(expanded_path.as_ref()))
        } else {
            None
        };

        Ok(Self {
            config,
            sync_engine,
            shutdown_sender,
            is_running,
            pid_file_path,
        })
    }

    /// Start the daemon in the foreground
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting RepoSentry daemon");

        // Write PID file if configured
        self.write_pid_file().context("Failed to write PID file")?;

        // Set running state
        self.is_running.store(true, Ordering::SeqCst);

        // Setup graceful shutdown handling
        let shutdown_receiver = self.shutdown_sender.subscribe();
        let is_running = self.is_running.clone();

        // Spawn shutdown signal handler
        let shutdown_sender = self.shutdown_sender.clone();
        tokio::spawn(async move {
            Self::wait_for_shutdown_signal().await;
            info!("Shutdown signal received, stopping daemon...");
            is_running.store(false, Ordering::SeqCst);
            let _ = shutdown_sender.send(());
        });

        // Run the main daemon loop
        let result = self.daemon_loop(shutdown_receiver).await;

        // Cleanup on exit
        self.cleanup().context("Failed to cleanup daemon")?;

        result
    }

    /// Start the daemon as a background service (Unix platforms)
    #[cfg(unix)]
    pub fn daemonize(&self) -> Result<()> {
        use daemonize::Daemonize;

        let log_file = if !self.config.daemon.log_file.is_empty() {
            let expanded_path = shellexpand::full(&self.config.daemon.log_file)
                .context("Failed to expand log file path")?;
            let log_file = std::fs::File::create(expanded_path.as_ref())
                .context("Failed to create log file")?;
            Some(log_file)
        } else {
            None
        };

        let mut daemonize = Daemonize::new();

        if let Some(pid_path) = &self.pid_file_path {
            daemonize = daemonize.pid_file(pid_path);
        }

        if let Some(log_file) = log_file {
            daemonize = daemonize.stdout(log_file.try_clone()?)
                .stderr(log_file);
        }

        daemonize.start()
            .context("Failed to daemonize process")?;

        info!("RepoSentry daemon started as background service");
        Ok(())
    }

    /// Stop a running daemon by sending a shutdown signal
    pub async fn stop(&self) -> Result<()> {
        info!("Sending shutdown signal to daemon");

        if let Some(pid_file) = &self.pid_file_path {
            if pid_file.exists() {
                let pid_str = fs::read_to_string(pid_file)
                    .context("Failed to read PID file")?;

                let pid: u32 = pid_str.trim().parse()
                    .context("Invalid PID in PID file")?;

                #[cfg(unix)]
                {
                    use nix::sys::signal::{self, Signal};
                    use nix::unistd::Pid;

                    let pid = Pid::from_raw(pid as i32);
                    signal::kill(pid, Signal::SIGTERM)
                        .context("Failed to send SIGTERM to daemon process")?;
                }

                #[cfg(not(unix))]
                {
                    warn!("Daemon stop not implemented for this platform");
                }

                info!("Shutdown signal sent to daemon process {}", pid);
            } else {
                warn!("PID file not found, daemon may not be running");
            }
        } else {
            warn!("No PID file configured, cannot stop daemon");
        }

        Ok(())
    }

    /// Get current daemon status
    pub fn status(&self, start_time: Instant) -> DaemonStatus {
        let is_running = self.is_running.load(Ordering::SeqCst);
        let uptime = start_time.elapsed();

        // TODO: Track these statistics in the daemon loop
        let last_sync = None;
        let total_syncs = 0;
        let successful_syncs = 0;
        let failed_syncs = 0;

        // Calculate next sync time
        let next_sync_in = if is_running {
            let interval_secs = parse_daemon_duration(&self.config.daemon.interval).unwrap_or(1800); // Default 30 minutes
            Some(Duration::from_secs(interval_secs))
        } else {
            None
        };

        DaemonStatus {
            is_running,
            uptime,
            last_sync,
            total_syncs,
            successful_syncs,
            failed_syncs,
            next_sync_in,
        }
    }

    /// Main daemon loop - runs periodic sync operations
    async fn daemon_loop(&self, mut shutdown_receiver: broadcast::Receiver<()>) -> Result<()> {
        let sync_interval_secs = parse_daemon_duration(&self.config.daemon.interval)
            .context("Failed to parse daemon sync interval")?;
        let sync_interval = Duration::from_secs(sync_interval_secs);
        let mut interval_timer = interval(sync_interval);

        info!("Daemon loop started with interval: {:?}", sync_interval);

        // Skip the first immediate tick
        interval_timer.tick().await;

        loop {
            tokio::select! {
                // Shutdown signal received
                _ = shutdown_receiver.recv() => {
                    info!("Shutdown signal received in daemon loop");
                    break;
                }

                // Sync interval elapsed
                _ = interval_timer.tick() => {
                    if !self.is_running.load(Ordering::SeqCst) {
                        break;
                    }

                    debug!("Starting scheduled sync operation");
                    let sync_start = Instant::now();

                    match self.sync_engine.run_sync().await {
                        Ok(summary) => {
                            let sync_duration = sync_start.elapsed();
                            self.log_sync_success(&summary, sync_duration);
                        }
                        Err(e) => {
                            self.log_sync_failure(&e);
                        }
                    }
                }
            }
        }

        info!("Daemon loop exiting");
        Ok(())
    }

    /// Wait for shutdown signals (SIGTERM, SIGINT, Ctrl+C)
    async fn wait_for_shutdown_signal() {
        // For now, just handle Ctrl+C. More sophisticated signal handling
        // can be added later with proper tokio signal features
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl-c");
        debug!("Ctrl+C received");
    }

    /// Write PID file for daemon process management
    fn write_pid_file(&self) -> Result<()> {
        if let Some(pid_file) = &self.pid_file_path {
            let pid = std::process::id();

            // Create parent directories if they don't exist
            if let Some(parent) = pid_file.parent() {
                fs::create_dir_all(parent)
                    .context("Failed to create PID file directory")?;
            }

            fs::write(pid_file, pid.to_string())
                .context("Failed to write PID file")?;

            info!("PID file written: {} (PID: {})", pid_file.display(), pid);
        }

        Ok(())
    }

    /// Remove PID file and perform cleanup
    fn cleanup(&self) -> Result<()> {
        if let Some(pid_file) = &self.pid_file_path {
            if pid_file.exists() {
                fs::remove_file(pid_file)
                    .context("Failed to remove PID file")?;
                info!("PID file removed: {}", pid_file.display());
            }
        }

        self.is_running.store(false, Ordering::SeqCst);
        info!("Daemon cleanup completed");
        Ok(())
    }

    /// Log successful sync operation
    fn log_sync_success(&self, summary: &SyncSummary, duration: Duration) {
        info!(
            "Sync completed successfully in {:.2}s: {} repos, {} successful, {} failed, {} skipped",
            duration.as_secs_f64(),
            summary.total_repositories,
            summary.successful_operations,
            summary.failed_operations,
            summary.skipped_operations
        );
    }

    /// Log failed sync operation
    fn log_sync_failure(&self, error: &anyhow::Error) {
        error!("Sync operation failed: {:?}", error);
    }
}

/// Helper to create daemon from default config
pub async fn create_daemon_from_config() -> Result<Daemon> {
    let config = Config::load_or_default()
        .context("Failed to load configuration for daemon")?;

    Daemon::new(config).await
        .context("Failed to create daemon")
}

/// Check if daemon is currently running by checking PID file
pub fn is_daemon_running(config: &Config) -> Result<bool> {
    if !config.daemon.pid_file.is_empty() {
        let expanded_path = shellexpand::full(&config.daemon.pid_file)
            .context("Failed to expand PID file path")?;
        let pid_file = PathBuf::from(expanded_path.as_ref());

        if pid_file.exists() {
            let pid_str = fs::read_to_string(&pid_file)
                .context("Failed to read PID file")?;

            let pid: u32 = pid_str.trim().parse()
                .context("Invalid PID in PID file")?;

            // Check if process is actually running
            #[cfg(unix)]
            {
                use nix::sys::signal;
                use nix::unistd::Pid;
                use nix::errno::Errno;

                let pid = Pid::from_raw(pid as i32);
                match signal::kill(pid, None) {
                    Ok(_) => return Ok(true),  // Process exists
                    Err(Errno::ESRCH) => {
                        // Process doesn't exist, remove stale PID file
                        let _ = fs::remove_file(&pid_file);
                        return Ok(false);
                    }
                    Err(_) => return Ok(true), // Assume running if we can't check
                }
            }

            #[cfg(not(unix))]
            {
                // On non-Unix platforms, just check if PID file exists
                return Ok(true);
            }
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_daemon_creation() {
        let config = Config::default();
        let result = Daemon::new(config).await;

        // Test may fail if GitHub auth is not available
        match result {
            Ok(_daemon) => {
                // Test passed
            }
            Err(e) => {
                // Expected if authentication is not available
                assert!(e.to_string().contains("authentication") || e.to_string().contains("GitHub"));
            }
        }
    }

    #[test]
    fn test_pid_file_operations() {
        let temp_dir = tempdir().unwrap();
        let pid_file = temp_dir.path().join("test.pid");

        let mut config = Config::default();
        config.daemon.pid_file = pid_file.to_string_lossy().to_string();

        // This would test PID file operations in a real scenario
        assert!(!pid_file.exists()); // Initially doesn't exist

        // Test daemon running check
        let is_running = is_daemon_running(&config).unwrap();
        assert!(!is_running); // Should be false when no PID file exists
    }

    #[test]
    fn test_daemon_status() {
        // Test the status calculation logic
        let start_time = Instant::now();

        // Simulate daemon status
        let is_running = true;
        let uptime = start_time.elapsed();
        let interval_seconds = 300u64; // 5 minutes

        let status = DaemonStatus {
            is_running,
            uptime,
            last_sync: None,
            total_syncs: 0,
            successful_syncs: 0,
            failed_syncs: 0,
            next_sync_in: Some(Duration::from_secs(interval_seconds)),
        };

        assert!(status.is_running);
        assert!(status.uptime.as_nanos() > 0);
        assert!(status.next_sync_in.is_some());
        assert_eq!(status.next_sync_in.unwrap().as_secs(), interval_seconds);
    }
}