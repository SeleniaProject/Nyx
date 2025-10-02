// Test harness for Nyx integration tests
//
// Provides infrastructure for:
// - Multi-node daemon simulation
// - Client connection management
// - Network condition simulation (latency, packet loss)
// - Resource cleanup and graceful shutdown
//
// Design principles:
// - Pure Rust implementation (NO C/C++ dependencies)
// - Automatic resource cleanup via Drop trait
// - Timeout-based test orchestration
// - Minimal external process dependencies

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Test result type alias
pub type TestResult<T> = Result<T>;

/// Configuration for a test daemon instance
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Bind address for the daemon
    pub bind_addr: SocketAddr,
    /// Path to nyx.toml config file (if any)
    pub config_path: Option<PathBuf>,
    /// Enable telemetry
    pub telemetry_enabled: bool,
    /// Custom environment variables
    pub env_vars: HashMap<String, String>,
    /// Working directory
    pub work_dir: Option<PathBuf>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:0".parse().unwrap(), // Random port
            config_path: None,
            telemetry_enabled: false,
            env_vars: HashMap::new(),
            work_dir: None,
        }
    }
}

/// Handle to a running daemon process
pub struct DaemonHandle {
    /// Process handle
    child: Arc<Mutex<Child>>,
    /// Daemon configuration
    config: DaemonConfig,
    /// Actual bind address (after port allocation)
    actual_addr: Arc<RwLock<Option<SocketAddr>>>,
    /// Daemon ID for logging
    id: String,
}

impl DaemonHandle {
    /// Spawn a new daemon process
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this daemon (for logging)
    /// * `config` - Daemon configuration
    ///
    /// # Returns
    /// Handle to the spawned daemon, or error if spawn fails
    pub async fn spawn(id: impl Into<String>, config: DaemonConfig) -> TestResult<Self> {
        let id = id.into();
        info!("Spawning daemon '{}' with config: {:?}", id, config);

        // Build command
        let mut cmd = Command::new("cargo");
        cmd.arg("run")
            .arg("--bin")
            .arg("nyx-daemon")
            .arg("--")
            .arg("--bind")
            .arg(config.bind_addr.to_string());

        // Add config file if specified
        if let Some(ref config_path) = config.config_path {
            cmd.arg("--config").arg(config_path);
        }

        // Disable telemetry unless explicitly enabled
        if !config.telemetry_enabled {
            cmd.env("NYX_TELEMETRY_DISABLED", "1");
        }

        // Add custom environment variables
        for (key, value) in &config.env_vars {
            cmd.env(key, value);
        }

        // Set working directory to workspace root (parent of tests/) if not explicitly specified
        // This ensures cargo run can find nyx-daemon binary
        if let Some(ref work_dir) = config.work_dir {
            cmd.current_dir(work_dir);
        } else {
            // Detect workspace root: assume tests/ is in workspace root
            let current_dir = std::env::current_dir()?;
            let workspace_root = if current_dir.ends_with("tests") {
                current_dir.parent().unwrap_or(&current_dir).to_path_buf()
            } else {
                current_dir
            };
            cmd.current_dir(workspace_root);
        }

        // Capture stdout/stderr for debugging
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        // Spawn process
        let mut child = cmd
            .spawn()
            .context(format!("Failed to spawn daemon '{}'", id))?;

        // Read stdout/stderr in background for logging
        let stdout = child.stdout.take().expect("Failed to capture stdout");
        let stderr = child.stderr.take().expect("Failed to capture stderr");

        let id_clone = id.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!("[daemon {}] {}", id_clone, line);
            }
        });

        let id_clone = id.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                warn!("[daemon {}] STDERR: {}", id_clone, line);
            }
        });

        let handle = Self {
            child: Arc::new(Mutex::new(child)),
            config: config.clone(),
            actual_addr: Arc::new(RwLock::new(None)),
            id: id.clone(),
        };

        // Wait for daemon to be ready (simple approach: try connecting)
        handle.wait_ready(Duration::from_secs(10)).await?;

        info!("Daemon '{}' spawned and ready", id);
        Ok(handle)
    }

    /// Wait for daemon to be ready by attempting to connect
    async fn wait_ready(&self, timeout_duration: Duration) -> TestResult<()> {
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout_duration {
                return Err(anyhow::anyhow!(
                    "Daemon '{}' did not become ready within {:?}",
                    self.id,
                    timeout_duration
                ));
            }

            // Try to connect
            match TcpStream::connect(&self.config.bind_addr).await {
                Ok(_stream) => {
                    // Connection succeeded, daemon is ready
                    let mut addr = self.actual_addr.write().await;
                    *addr = Some(self.config.bind_addr);
                    return Ok(());
                }
                Err(_) => {
                    // Connection failed, wait and retry
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    }

    /// Get the actual bind address of the daemon
    pub async fn bind_addr(&self) -> Option<SocketAddr> {
        *self.actual_addr.read().await
    }

    /// Get the daemon ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Gracefully shutdown the daemon
    pub async fn shutdown(&self) -> TestResult<()> {
        info!("Shutting down daemon '{}'", self.id);

        let mut child = self.child.lock().await;

        // Attempt graceful shutdown first, then force kill if needed
        // On both Unix and Windows, we use kill() for simplicity
        // (Production code should use proper signal handling)
        let _ = child.start_kill();

        // Wait for process to exit (with timeout)
        match timeout(Duration::from_secs(5), child.wait()).await {
            Ok(Ok(status)) => {
                info!("Daemon '{}' exited with status: {:?}", self.id, status);
                Ok(())
            }
            Ok(Err(e)) => Err(anyhow::anyhow!(
                "Failed to wait for daemon '{}': {}",
                self.id,
                e
            )),
            Err(_) => {
                warn!("Daemon '{}' did not exit within timeout, forcing kill", self.id);
                child.kill().await?;
                Ok(())
            }
        }
    }
}

impl Drop for DaemonHandle {
    fn drop(&mut self) {
        // Best-effort cleanup
        // Note: Drop is not async, so we spawn a blocking task
        let child = Arc::clone(&self.child);
        let id = self.id.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let mut child = child.lock().await;
                if let Err(e) = child.kill().await {
                    error!("Failed to kill daemon '{}' on drop: {}", id, e);
                }
            });
        });
    }
}

/// Handle to a client connection
pub struct ClientHandle {
    /// TCP stream to daemon
    stream: Arc<Mutex<TcpStream>>,
    /// Client ID for logging
    id: String,
    /// Connected daemon address
    daemon_addr: SocketAddr,
}

impl ClientHandle {
    /// Connect to a daemon
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this client (for logging)
    /// * `daemon_addr` - Address of the daemon to connect to
    ///
    /// # Returns
    /// Handle to the client connection, or error if connection fails
    pub async fn connect(
        id: impl Into<String>,
        daemon_addr: SocketAddr,
    ) -> TestResult<Self> {
        let id = id.into();
        info!("Connecting client '{}' to daemon at {}", id, daemon_addr);

        let stream = timeout(Duration::from_secs(5), TcpStream::connect(daemon_addr))
            .await
            .context("Connection timeout")?
            .context(format!("Failed to connect to daemon at {}", daemon_addr))?;

        info!("Client '{}' connected to {}", id, daemon_addr);

        Ok(Self {
            stream: Arc::new(Mutex::new(stream)),
            id,
            daemon_addr,
        })
    }

    /// Get the client ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Get the connected daemon address
    pub fn daemon_addr(&self) -> SocketAddr {
        self.daemon_addr
    }

    /// Send data to the daemon
    pub async fn send(&self, data: &[u8]) -> TestResult<()> {
        let mut stream = self.stream.lock().await;
        stream
            .write_all(data)
            .await
            .context("Failed to send data")?;
        stream.flush().await.context("Failed to flush stream")?;
        Ok(())
    }

    /// Receive data from the daemon
    pub async fn recv(&self, buf: &mut [u8]) -> TestResult<usize> {
        let mut stream = self.stream.lock().await;
        tokio::io::AsyncReadExt::read(&mut *stream, buf)
            .await
            .context("Failed to receive data")
    }

    /// Close the connection
    pub async fn close(&self) -> TestResult<()> {
        let mut stream = self.stream.lock().await;
        stream.shutdown().await.context("Failed to shutdown stream")
    }
}

/// Network simulation configuration
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Latency (mean) in milliseconds
    pub latency_ms: u64,
    /// Jitter (latency variation) in milliseconds
    pub jitter_ms: u64,
    /// Packet loss rate (0.0 = no loss, 1.0 = 100% loss)
    pub loss_rate: f64,
    /// Bandwidth limit in bytes per second (None = unlimited)
    pub bandwidth_bps: Option<u64>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            latency_ms: 0,
            jitter_ms: 0,
            loss_rate: 0.0,
            bandwidth_bps: None,
        }
    }
}

impl NetworkConfig {
    /// Create configuration for good network conditions
    pub fn good() -> Self {
        Self {
            latency_ms: 20,
            jitter_ms: 5,
            loss_rate: 0.001, // 0.1%
            bandwidth_bps: Some(10_000_000), // 10 Mbps
        }
    }

    /// Create configuration for poor network conditions
    pub fn poor() -> Self {
        Self {
            latency_ms: 200,
            jitter_ms: 50,
            loss_rate: 0.05, // 5%
            bandwidth_bps: Some(1_000_000), // 1 Mbps
        }
    }

    /// Create configuration for unstable network conditions
    pub fn unstable() -> Self {
        Self {
            latency_ms: 100,
            jitter_ms: 100, // High jitter
            loss_rate: 0.1, // 10%
            bandwidth_bps: Some(5_000_000), // 5 Mbps
        }
    }
}

/// Test network for simulating network conditions
pub struct TestNetwork {
    config: NetworkConfig,
}

impl TestNetwork {
    /// Create a new test network with given configuration
    pub fn new(config: NetworkConfig) -> Self {
        Self { config }
    }

    /// Create a test network with ideal conditions (no latency, no loss)
    pub fn ideal() -> Self {
        Self::new(NetworkConfig::default())
    }

    /// Simulate network delay with jitter
    pub async fn simulate_delay(&self) {
        let base_latency = self.config.latency_ms;
        let jitter = self.config.jitter_ms;

        if base_latency > 0 || jitter > 0 {
            // Apply jitter: random variation in range [-jitter, +jitter]
            let jitter_offset = if jitter > 0 {
                let random_jitter = rand::random::<f64>() * 2.0 - 1.0; // -1.0 to 1.0
                (random_jitter * jitter as f64) as i64
            } else {
                0
            };

            let actual_latency = ((base_latency as i64) + jitter_offset).max(0) as u64;
            tokio::time::sleep(Duration::from_millis(actual_latency)).await;
        }
    }

    /// Get network configuration
    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }

    /// Check if a packet should be dropped (based on loss rate)
    pub fn should_drop_packet(&self) -> bool {
        if self.config.loss_rate <= 0.0 {
            return false;
        }
        rand::random::<f64>() < self.config.loss_rate
    }
}

/// Test harness for orchestrating integration tests
pub struct TestHarness {
    /// Daemons managed by this harness
    daemons: HashMap<String, DaemonHandle>,
    /// Clients managed by this harness
    clients: HashMap<String, ClientHandle>,
    /// Test network configuration
    #[allow(dead_code)] // Will be used in future network simulation tests
    network: TestNetwork,
}

impl TestHarness {
    /// Create a new test harness with ideal network conditions
    pub fn new() -> Self {
        Self {
            daemons: HashMap::new(),
            clients: HashMap::new(),
            network: TestNetwork::ideal(),
        }
    }

    /// Create a new test harness with custom network configuration
    pub fn with_network(network_config: NetworkConfig) -> Self {
        Self {
            daemons: HashMap::new(),
            clients: HashMap::new(),
            network: TestNetwork::new(network_config),
        }
    }

    /// Spawn a daemon and add it to the harness
    pub async fn spawn_daemon(
        &mut self,
        id: impl Into<String>,
        config: DaemonConfig,
    ) -> TestResult<()> {
        let id = id.into();
        let daemon = DaemonHandle::spawn(&id, config).await?;
        self.daemons.insert(id, daemon);
        Ok(())
    }

    /// Connect a client to a daemon
    pub async fn connect_client(
        &mut self,
        client_id: impl Into<String>,
        daemon_id: &str,
    ) -> TestResult<()> {
        let client_id = client_id.into();

        let daemon = self
            .daemons
            .get(daemon_id)
            .ok_or_else(|| anyhow::anyhow!("Daemon '{}' not found", daemon_id))?;

        let daemon_addr = daemon
            .bind_addr()
            .await
            .ok_or_else(|| anyhow::anyhow!("Daemon '{}' has no bind address", daemon_id))?;

        let client = ClientHandle::connect(&client_id, daemon_addr).await?;
        self.clients.insert(client_id, client);
        Ok(())
    }

    /// Get a daemon handle by ID
    pub fn daemon(&self, id: &str) -> Option<&DaemonHandle> {
        self.daemons.get(id)
    }

    /// Get a client handle by ID
    pub fn client(&self, id: &str) -> Option<&ClientHandle> {
        self.clients.get(id)
    }

    /// Shutdown all daemons and close all clients
    pub async fn shutdown_all(&mut self) -> TestResult<()> {
        info!("Shutting down all daemons and clients");

        // Close all clients
        for (id, client) in self.clients.drain() {
            if let Err(e) = client.close().await {
                warn!("Failed to close client '{}': {}", id, e);
            }
        }

        // Shutdown all daemons
        for (id, daemon) in self.daemons.drain() {
            if let Err(e) = daemon.shutdown().await {
                error!("Failed to shutdown daemon '{}': {}", id, e);
            }
        }

        Ok(())
    }
}

impl Drop for TestHarness {
    fn drop(&mut self) {
        // Best-effort cleanup
        let daemons: Vec<_> = self.daemons.drain().map(|(_, d)| d).collect();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                for daemon in daemons {
                    if let Err(e) = daemon.shutdown().await {
                        error!("Failed to shutdown daemon on drop: {}", e);
                    }
                }
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        assert!(!config.telemetry_enabled);
        assert!(config.config_path.is_none());
        assert!(config.env_vars.is_empty());
    }

    #[tokio::test]
    async fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert_eq!(config.latency_ms, 0);
        assert_eq!(config.loss_rate, 0.0);
        assert!(config.bandwidth_bps.is_none());
    }

    #[tokio::test]
    async fn test_test_network_ideal() {
        let network = TestNetwork::ideal();
        assert!(!network.should_drop_packet());
        // simulate_delay should return immediately
        let start = std::time::Instant::now();
        network.simulate_delay().await;
        assert!(start.elapsed() < Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_test_harness_creation() {
        let harness = TestHarness::new();
        assert_eq!(harness.daemons.len(), 0);
        assert_eq!(harness.clients.len(), 0);
    }
}
