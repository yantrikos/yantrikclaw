//! Lifecycle manager for the Yantrik Companion binary.
//!
//! When `manage_process: true` in `CompanionConfig`, ZeroClaw spawns the
//! companion binary as a child process, monitors its health, and restarts
//! it on failure. On shutdown, it sends a graceful termination signal.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

/// How long to wait for the companion to become healthy after starting.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
/// Delay between health check retries during startup.
const STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(500);
/// Interval between background health checks.
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);
/// Number of consecutive health check failures before triggering a restart.
const MAX_CONSECUTIVE_FAILURES: u32 = 3;
/// Grace period after sending kill signal before force-killing.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Manages the companion binary's lifecycle.
pub struct CompanionProcess {
    binary_path: PathBuf,
    config_path: Option<PathBuf>,
    health_url: String,
    child: Arc<Mutex<Option<Child>>>,
    client: Client,
}

impl CompanionProcess {
    pub fn new(
        binary_path: PathBuf,
        config_path: Option<PathBuf>,
        companion_url: &str,
    ) -> Self {
        Self {
            binary_path,
            config_path,
            health_url: format!("{companion_url}/health"),
            child: Arc::new(Mutex::new(None)),
            client: Client::new(),
        }
    }

    /// Start the companion binary as a child process.
    ///
    /// Spawns `<binary_path> serve [--config <config_path>]` and waits for
    /// the `/health` endpoint to return `ok: true`.
    pub async fn start(&self) -> Result<(), String> {
        let mut guard = self.child.lock().await;

        // Check if already running.
        if let Some(ref mut child) = *guard {
            match child.try_wait() {
                Ok(None) => return Ok(()), // still running
                Ok(Some(status)) => {
                    warn!("companion process exited with {status}, restarting");
                }
                Err(e) => {
                    warn!("failed to check companion status: {e}");
                }
            }
        }

        let mut cmd = Command::new(&self.binary_path);
        cmd.arg("serve");
        if let Some(ref config) = self.config_path {
            cmd.arg("--config").arg(config);
        }

        // Inherit stdout/stderr so companion logs are visible.
        cmd.stdout(std::process::Stdio::inherit());
        cmd.stderr(std::process::Stdio::inherit());

        info!(
            "starting companion: {} serve{}",
            self.binary_path.display(),
            self.config_path
                .as_ref()
                .map(|p| format!(" --config {}", p.display()))
                .unwrap_or_default(),
        );

        let child = cmd
            .spawn()
            .map_err(|e| format!("failed to spawn companion: {e}"))?;

        *guard = Some(child);
        drop(guard); // release lock during health check wait

        // Wait for health endpoint to respond.
        self.wait_for_healthy().await?;

        info!("companion is healthy");
        Ok(())
    }

    /// Stop the companion process gracefully.
    pub async fn stop(&self) -> Result<(), String> {
        let mut guard = self.child.lock().await;
        if let Some(mut child) = guard.take() {
            info!("stopping companion process");

            // On Unix, send SIGTERM for graceful shutdown.
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;
                if let Some(pid) = child.id() {
                    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                }
            }

            // On Windows, use kill() directly (sends TerminateProcess).
            #[cfg(windows)]
            {
                let _ = child.start_kill();
            }

            // Wait with timeout for graceful exit.
            match tokio::time::timeout(SHUTDOWN_GRACE, child.wait()).await {
                Ok(Ok(status)) => {
                    info!("companion exited: {status}");
                }
                Ok(Err(e)) => {
                    warn!("error waiting for companion exit: {e}");
                }
                Err(_) => {
                    warn!("companion did not exit in time, force killing");
                    let _ = child.kill().await;
                }
            }
        }
        Ok(())
    }

    /// Check if the companion is healthy via `GET /health`.
    pub async fn is_healthy(&self) -> bool {
        match self
            .client
            .get(&self.health_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
        {
            Ok(res) if res.status().is_success() => {
                #[derive(serde::Deserialize)]
                struct H {
                    ok: bool,
                }
                res.json::<H>().await.map(|h| h.ok).unwrap_or(false)
            }
            _ => false,
        }
    }

    /// Wait for the companion to become healthy after startup.
    async fn wait_for_healthy(&self) -> Result<(), String> {
        let deadline = tokio::time::Instant::now() + STARTUP_TIMEOUT;

        loop {
            if tokio::time::Instant::now() >= deadline {
                return Err(format!(
                    "companion did not become healthy within {}s",
                    STARTUP_TIMEOUT.as_secs(),
                ));
            }

            if self.is_healthy().await {
                return Ok(());
            }

            // Check if child crashed during startup.
            let mut guard = self.child.lock().await;
            if let Some(ref mut child) = *guard {
                if let Ok(Some(status)) = child.try_wait() {
                    return Err(format!("companion exited during startup: {status}"));
                }
            }
            drop(guard);

            tokio::time::sleep(STARTUP_POLL_INTERVAL).await;
        }
    }

    /// Spawn a background task that monitors health and restarts on failure.
    pub fn spawn_health_monitor(self: &Arc<Self>) {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut consecutive_failures: u32 = 0;
            let mut interval = tokio::time::interval(HEALTH_CHECK_INTERVAL);
            // Skip initial tick.
            interval.tick().await;

            loop {
                interval.tick().await;

                if this.is_healthy().await {
                    if consecutive_failures > 0 {
                        info!(
                            "companion health recovered after {consecutive_failures} failures"
                        );
                        consecutive_failures = 0;
                    }
                } else {
                    consecutive_failures += 1;
                    warn!(
                        "companion health check failed ({consecutive_failures}/{})",
                        MAX_CONSECUTIVE_FAILURES,
                    );

                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        error!("companion unresponsive, attempting restart");
                        if let Err(e) = this.stop().await {
                            warn!("failed to stop companion: {e}");
                        }
                        match this.start().await {
                            Ok(()) => {
                                info!("companion restarted successfully");
                                consecutive_failures = 0;
                            }
                            Err(e) => {
                                error!("companion restart failed: {e}");
                                // Keep trying on next health check cycle.
                            }
                        }
                    }
                }
            }
        });
    }
}

impl Drop for CompanionProcess {
    fn drop(&mut self) {
        // Best-effort synchronous kill on drop.
        if let Ok(mut guard) = self.child.try_lock() {
            if let Some(ref mut child) = *guard {
                let _ = child.start_kill();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_correct_health_url() {
        let proc = CompanionProcess::new(
            PathBuf::from("/usr/local/bin/yantrik"),
            Some(PathBuf::from("/etc/yantrik/config.yaml")),
            "http://127.0.0.1:8000",
        );
        assert_eq!(proc.health_url, "http://127.0.0.1:8000/health");
    }

    #[test]
    fn new_without_config_path() {
        let proc = CompanionProcess::new(
            PathBuf::from("yantrik"),
            None,
            "http://localhost:9000",
        );
        assert!(proc.config_path.is_none());
        assert_eq!(proc.health_url, "http://localhost:9000/health");
    }
}
