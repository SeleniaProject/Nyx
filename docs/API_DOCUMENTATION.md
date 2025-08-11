# NyxNet v1.0 - Complete API Documentation

## Table of Contents
1. [Overview](#overview)
2. [Core APIs](#core-apis)
3. [Transport Layer](#transport-layer)
4. [Cryptographic APIs](#cryptographic-apis)
5. [Stream Management](#stream-management)
6. [Advanced Features](#advanced-features)
7. [Error Handling](#error-handling)
8. [Examples](#examples)

## Overview

NyxNet v1.0 provides a comprehensive anonymous communication framework with the following key features:

- **Hybrid Post-Quantum Cryptography**: Kyber1024 + X25519
- **Multipath Routing**: Up to 8 concurrent paths with weighted round-robin
- **Low Power Mode**: Mobile-optimized with push notification support
- **TCP Fallback**: Reliable transport for restrictive networks
- **Plugin Framework**: Extensible architecture for custom protocols
- **Advanced Performance**: Zero-copy buffers, adaptive optimization

## Core APIs

### NyxConfig

Primary configuration structure for NyxNet nodes.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NyxConfig {
    pub node_id: String,
    pub listen_port: u16,
    pub data_dir: PathBuf,
    pub max_connections: usize,
    pub enable_cover_traffic: bool,
    pub cover_traffic_ratio: f32,
    pub multipath: MultipathConfig,
    pub low_power: LowPowerConfig,
    pub push_config: Option<PushConfig>,
    pub tcp_fallback: TcpFallbackConfig,
    pub performance: PerformanceConfig,
}

impl NyxConfig {
    /// Create a new configuration with sensible defaults
    pub fn new() -> Self;
    
    /// Load configuration from TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, NyxError>;
    
    /// Save configuration to TOML file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), NyxError>;
    
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<(), NyxError>;
    
    /// Apply environment variable overrides
    pub fn apply_env_overrides(&mut self);
}
```

**Example Usage:**

```rust
use nyx_core::NyxConfig;

// Create default configuration
let mut config = NyxConfig::new();
config.node_id = "my-node".to_string();
config.listen_port = 44380;

// Enable multipath with 4 concurrent paths
config.multipath.enabled = true;
config.multipath.max_paths = 4;

// Configure low power mode for mobile
config.low_power.enabled = true;
config.low_power.screen_off_ratio = 0.1;
config.push_config = Some(PushConfig {
    fcm_server_key: "your-fcm-key".to_string(),
    // ... other push config
});

// Save configuration
config.to_file("nyx.toml")?;
```

### NyxDaemon

Main daemon process for running a Nyx node.

```rust
pub struct NyxDaemon {
    config: NyxConfig,
    transport: Arc<dyn Transport>,
    stream_manager: StreamManager,
    router: AdvancedRouter,
    performance_optimizer: PerformanceOptimizer,
    low_power_manager: Option<LowPowerManager>,
}

impl NyxDaemon {
    /// Create a new daemon instance
    pub async fn new(config: NyxConfig) -> Result<Self, NyxError>;
    
    /// Start the daemon (non-blocking)
    pub async fn start(&mut self) -> Result<(), NyxError>;
    
    /// Run the daemon (blocking until shutdown)
    pub async fn run(&self) -> Result<(), NyxError>;
    
    /// Graceful shutdown
    pub async fn shutdown(&self) -> Result<(), NyxError>;
    
    /// Get current node statistics
    pub async fn get_stats(&self) -> NodeStats;
    
    /// Get list of active connections
    pub async fn get_connections(&self) -> Vec<ConnectionInfo>;
    
    /// Force garbage collection and cleanup
    pub async fn force_cleanup(&self) -> Result<CleanupStats, NyxError>;
}

#[derive(Debug, Clone)]
pub struct NodeStats {
    pub uptime: Duration,
    pub total_connections: u64,
    pub active_connections: u32,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub error_count: u32,
    pub multipath_stats: MultipathStats,
    pub performance_metrics: PerformanceMetrics,
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub connection_id: ConnectionId,
    pub peer_endpoint: NodeEndpoint,
    pub established_at: Instant,
    pub last_activity: Instant,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub status: ConnectionStatus,
    pub path_id: Option<PathId>,
}
```

**Example Usage:**

```rust
use nyx_daemon::NyxDaemon;
use nyx_core::NyxConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = NyxConfig::from_file("nyx.toml")?;
    
    // Create and start daemon
    let mut daemon = NyxDaemon::new(config).await?;
    
    // Start background services
    daemon.start().await?;
    
    // Run until shutdown signal
    tokio::select! {
        result = daemon.run() => {
            if let Err(e) = result {
                eprintln!("Daemon error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl+C, shutting down...");
            daemon.shutdown().await?;
        }
    }
    
    Ok(())
}
```

### NyxClient

High-level client API for interacting with Nyx network.

```rust
pub struct NyxClient {
    config: ClientConfig,
    transport: Arc<dyn Transport>,
    session_manager: SessionManager,
}

impl NyxClient {
    /// Create a new client
    pub async fn new(config: ClientConfig) -> Result<Self, NyxError>;
    
    /// Connect to a peer by NodeId
    pub async fn connect(&self, peer_id: &NodeId) -> Result<ConnectionHandle, NyxError>;
    
    /// Send a message to a peer
    pub async fn send_message(
        &self,
        peer_id: &NodeId,
        message: &[u8]
    ) -> Result<MessageId, NyxError>;
    
    /// Send a message with delivery confirmation
    pub async fn send_message_confirmed(
        &self,
        peer_id: &NodeId,
        message: &[u8],
        timeout: Duration
    ) -> Result<DeliveryReceipt, NyxError>;
    
    /// Receive messages from a specific peer
    pub async fn receive_messages(
        &self,
        peer_id: &NodeId
    ) -> Result<MessageStream, NyxError>;
    
    /// Subscribe to all incoming messages
    pub fn message_stream(&self) -> MessageStream;
    
    /// Get client statistics
    pub async fn get_stats(&self) -> ClientStats;
    
    /// Disconnect from a peer
    pub async fn disconnect(&self, peer_id: &NodeId) -> Result<(), NyxError>;
    
    /// Shutdown client
    pub async fn shutdown(self) -> Result<(), NyxError>;
}

#[derive(Debug)]
pub struct ConnectionHandle {
    pub connection_id: ConnectionId,
    pub peer_id: NodeId,
    pub established_at: Instant,
}

#[derive(Debug)]
pub struct DeliveryReceipt {
    pub message_id: MessageId,
    pub delivered_at: Instant,
    pub path_used: PathId,
    pub delivery_time: Duration,
}

pub type MessageStream = Pin<Box<dyn Stream<Item = IncomingMessage> + Send>>;

#[derive(Debug)]
pub struct IncomingMessage {
    pub from: NodeId,
    pub data: Vec<u8>,
    pub received_at: Instant,
    pub path_id: PathId,
    pub message_id: MessageId,
}
```

**Example Usage:**

```rust
use nyx_client::{NyxClient, ClientConfig};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ClientConfig {
        node_id: "client-1".to_string(),
        bootstrap_nodes: vec!["127.0.0.1:44380".parse()?],
        ..Default::default()
    };
    
    let client = NyxClient::new(config).await?;
    
    // Connect to a peer
    let peer_id = NodeId::from_hex("abcd...")?;
    let connection = client.connect(&peer_id).await?;
    println!("Connected to peer: {:?}", connection);
    
    // Send a message
    let message_id = client.send_message(&peer_id, b"Hello, Nyx!").await?;
    println!("Sent message: {:?}", message_id);
    
    // Listen for incoming messages
    let mut message_stream = client.message_stream();
    while let Some(message) = message_stream.next().await {
        println!("Received from {}: {:?}", 
                 message.from, 
                 String::from_utf8_lossy(&message.data));
    }
    
    Ok(())
}
```

## Transport Layer

### UdpTransport

Primary UDP-based transport with NAT traversal capabilities.

```rust
pub struct UdpTransport {
    config: UdpConfig,
    socket: UdpSocket,
    nat_traversal: NatTraversal,
    packet_buffer: Arc<RwLock<PacketBuffer>>,
}

impl Transport for UdpTransport {
    async fn send(
        &self,
        destination: SocketAddr,
        data: &[u8]
    ) -> Result<(), TransportError>;
    
    async fn receive(&self) -> Result<(SocketAddr, Vec<u8>), TransportError>;
    
    async fn bind(addr: SocketAddr) -> Result<Self, TransportError>;
    
    fn local_addr(&self) -> Result<SocketAddr, TransportError>;
    
    async fn close(self) -> Result<(), TransportError>;
}

impl UdpTransport {
    /// Create transport with automatic NAT traversal
    pub async fn new_with_nat_traversal(
        bind_addr: SocketAddr,
        nat_config: NatTraversalConfig
    ) -> Result<Self, TransportError>;
    
    /// Get NAT traversal statistics
    pub async fn nat_stats(&self) -> NatTraversalStats;
    
    /// Perform ICE connectivity checks
    pub async fn ice_connectivity_check(
        &self,
        remote_candidates: &[SocketAddr]
    ) -> Result<SocketAddr, TransportError>;
    
    /// Enable hole punching for specific peer
    pub async fn enable_hole_punching(
        &self,
        peer_addr: SocketAddr
    ) -> Result<(), TransportError>;
}
```

### TcpFallbackTransport

TCP fallback transport for restrictive networks.

```rust
pub struct TcpFallbackTransport {
    config: TcpFallbackConfig,
    connections: Arc<RwLock<HashMap<NodeEndpoint, Arc<Mutex<TcpConnection>>>>>,
    connection_pool: Arc<RwLock<ConnectionPool>>,
}

impl TcpFallbackTransport {
    /// Create new TCP fallback transport
    pub fn new(config: TcpFallbackConfig) -> Self;
    
    /// Connect to endpoint with retry logic
    pub async fn connect_with_retry(
        addr: &str,
        config: TcpFallbackConfig
    ) -> Result<Self, TcpFallbackError>;
    
    /// Connect via SOCKS5 proxy
    pub async fn connect_via_socks5(
        &self,
        endpoint: &NodeEndpoint,
        proxy_addr: &str,
        credentials: Option<(&str, &str)>
    ) -> Result<Arc<Mutex<TcpConnection>>, TcpFallbackError>;
    
    /// Get connection statistics
    pub async fn get_stats(&self) -> HashMap<NodeEndpoint, ConnectionStats>;
    
    /// Force cleanup of idle connections
    pub async fn cleanup_idle_connections(&self, max_idle: Duration);
}

#[derive(Debug, Clone)]
pub struct TcpFallbackConfig {
    pub enabled: bool,
    pub connect_timeout: Duration,
    pub keepalive_interval: Duration,
    pub max_idle_time: Duration,
    pub buffer_size: usize,
    pub max_connections: usize,
    pub retry_attempts: u32,
    pub retry_backoff: Duration,
    pub proxy_support: Option<ProxyConfig>,
}

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub proxy_type: ProxyType,
    pub address: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub connect_timeout: Duration,
}

#[derive(Debug, Clone)]
pub enum ProxyType {
    Http,
    Socks5,
    Socks4,
}
```

## Cryptographic APIs

### Hybrid Post-Quantum Handshake

```rust
pub struct HybridNoiseHandshake {
    state: HandshakeState,
    pattern: HandshakePattern,
    classical_keypair: (PrivateKey, PublicKey), // X25519
    pq_keypair: (KyberPrivateKey, KyberPublicKey), // Kyber1024
    hybrid_secret: Option<[u8; 64]>, // Combined shared secret
}

impl HybridNoiseHandshake {
    /// Create new hybrid handshake (initiator)
    pub fn new_initiator() -> Result<Self, NoiseError>;
    
    /// Create new hybrid handshake (responder)
    pub fn new_responder() -> Result<Self, NoiseError>;
    
    /// Process handshake message
    pub fn process_message(
        &mut self,
        message: &[u8]
    ) -> Result<Option<Vec<u8>>, NoiseError>;
    
    /// Check if handshake is complete
    pub fn is_complete(&self) -> bool;
    
    /// Extract session keys after completed handshake
    pub fn extract_session_keys(self) -> Result<(SessionKey, SessionKey), NoiseError>;
    
    /// Get handshake pattern being used
    pub fn pattern(&self) -> &HandshakePattern;
}

/// Session key for encrypted communication
pub struct SessionKey {
    key: [u8; 32],
    nonce_counter: AtomicU64,
    created_at: Instant,
}

impl SessionKey {
    /// Encrypt data with this session key
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError>;
    
    /// Decrypt data with this session key
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError>;
    
    /// Check if key needs rotation
    pub fn needs_rotation(&self, max_age: Duration) -> bool;
    
    /// Get key age
    pub fn age(&self) -> Duration;
    
    /// Derive new key from this key (for forward secrecy)
    pub fn derive_next(&self) -> Result<SessionKey, CryptoError>;
}
```

### Post-Quantum Key Exchange

```rust
/// Kyber1024 key encapsulation mechanism
pub struct KyberKem {
    private_key: KyberPrivateKey,
    public_key: KyberPublicKey,
}

impl KyberKem {
    /// Generate new Kyber keypair
    pub fn generate() -> Result<Self, CryptoError>;
    
    /// Import from existing keys
    pub fn from_keys(
        private_key: KyberPrivateKey,
        public_key: KyberPublicKey
    ) -> Self;
    
    /// Encapsulate (generate shared secret + ciphertext)
    pub fn encapsulate(
        public_key: &KyberPublicKey
    ) -> Result<(SharedSecret, Ciphertext), CryptoError>;
    
    /// Decapsulate (recover shared secret from ciphertext)
    pub fn decapsulate(
        &self,
        ciphertext: &Ciphertext
    ) -> Result<SharedSecret, CryptoError>;
    
    /// Export public key for transmission
    pub fn public_key_bytes(&self) -> &[u8; KYBER_PUBLIC_KEY_BYTES];
    
    /// Import public key from bytes
    pub fn import_public_key(bytes: &[u8]) -> Result<KyberPublicKey, CryptoError>;
}

/// BIKE alternative for PQ-only mode
pub struct BikeKem {
    private_key: BikePrivateKey,
    public_key: BikePublicKey,
}

impl BikeKem {
    /// Generate new BIKE keypair
    pub fn generate() -> Result<Self, CryptoError>;
    
    /// Encapsulate with BIKE
    pub fn encapsulate(
        public_key: &BikePublicKey
    ) -> Result<(SharedSecret, Ciphertext), CryptoError>;
    
    /// Decapsulate with BIKE
    pub fn decapsulate(
        &self,
        ciphertext: &Ciphertext
    ) -> Result<SharedSecret, CryptoError>;
}
```

## Stream Management

### StreamManager

Manages bidirectional streams with flow control and error recovery.

```rust
pub struct StreamManager {
    config: StreamConfig,
    streams: Arc<RwLock<HashMap<StreamId, Arc<Stream>>>>,
    flow_controller: CongestionController,
    reorder_buffer: ReorderingBuffer,
    multipath_manager: MultipathManager,
}

impl StreamManager {
    /// Create new stream manager
    pub fn new(config: StreamConfig) -> Self;
    
    /// Create a new outbound stream
    pub async fn create_stream(
        &self,
        destination: NodeId,
        stream_type: StreamType
    ) -> Result<Arc<Stream>, StreamError>;
    
    /// Accept an inbound stream
    pub async fn accept_stream(&self) -> Result<Arc<Stream>, StreamError>;
    
    /// Get stream by ID
    pub async fn get_stream(&self, stream_id: StreamId) -> Option<Arc<Stream>>;
    
    /// Close a stream
    pub async fn close_stream(&self, stream_id: StreamId) -> Result<(), StreamError>;
    
    /// Get stream statistics
    pub async fn get_stats(&self) -> StreamManagerStats;
    
    /// Force close all streams
    pub async fn close_all_streams(&self);
}

/// Individual stream for bidirectional communication
pub struct Stream {
    stream_id: StreamId,
    peer_id: NodeId,
    stream_type: StreamType,
    state: Arc<RwLock<StreamState>>,
    send_queue: Arc<Mutex<VecDeque<StreamFrame>>>,
    receive_buffer: Arc<Mutex<ReceiveBuffer>>,
    flow_control: FlowController,
}

impl Stream {
    /// Send data on this stream
    pub async fn send(&self, data: &[u8]) -> Result<(), StreamError>;
    
    /// Receive data from this stream
    pub async fn receive(&self) -> Result<Vec<u8>, StreamError>;
    
    /// Send data with priority
    pub async fn send_with_priority(
        &self,
        data: &[u8],
        priority: Priority
    ) -> Result<(), StreamError>;
    
    /// Close the stream gracefully
    pub async fn close(&self) -> Result<(), StreamError>;
    
    /// Reset the stream (abort)
    pub async fn reset(&self) -> Result<(), StreamError>;
    
    /// Get stream state
    pub async fn state(&self) -> StreamState;
    
    /// Get stream statistics
    pub async fn get_stats(&self) -> StreamStats;
}

#[derive(Debug, Clone)]
pub enum StreamType {
    Reliable,          // TCP-like reliability
    Unreliable,        // UDP-like, best-effort
    ReliableOrdered,   // Reliable + in-order delivery
    UnreliableOrdered, // Best-effort + in-order
}

#[derive(Debug, Clone)]
pub enum StreamState {
    Connecting,
    Connected,
    Closing,
    Closed,
    Error(StreamError),
}

#[derive(Debug, Clone)]
pub struct StreamStats {
    pub stream_id: StreamId,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub retransmissions: u32,
    pub rtt_estimate: Duration,
    pub congestion_window: u32,
    pub flow_control_window: u32,
}
```

## Advanced Features

### Multipath Manager

```rust
pub use nyx_core::advanced_routing::{
    AdvancedRouter,
    RoutingAlgorithm,
    PathQuality,
    AdvancedRoutingConfig,
    RoutingStats,
    RoutingError,
};

impl AdvancedRouter {
    /// Create router with specified algorithm
    pub fn new(config: AdvancedRoutingConfig) -> Self;
    
    /// Add a new path for routing
    pub async fn add_path(&self, endpoint: NodeEndpoint) -> Result<(), RoutingError>;
    
    /// Remove a path from routing table
    pub async fn remove_path(&self, endpoint: &NodeEndpoint) -> Result<(), RoutingError>;
    
    /// Select best path based on current algorithm
    pub async fn select_path(&self) -> Result<NodeEndpoint, RoutingError>;
    
    /// Update path quality metrics
    pub async fn update_path_quality(
        &self,
        endpoint: &NodeEndpoint,
        quality: PathQuality
    ) -> Result<(), RoutingError>;
    
    /// Get routing statistics
    pub async fn get_routing_stats(&self) -> RoutingStats;
    
    /// Start background quality monitoring
    pub async fn start_monitoring(&self) -> Result<(), RoutingError>;
}

// See advanced_routing.rs for complete PathQuality and RoutingAlgorithm definitions
```

### Low Power Manager

```rust
pub use nyx_core::low_power::{
    LowPowerManager,
    PowerState,
    LowPowerConfig,
    DelayedMessage,
    PushNotificationService,
    LowPowerError,
};

impl LowPowerManager {
    /// Create new low power manager
    pub fn new(config: LowPowerConfig) -> Result<Self, LowPowerError>;
    
    /// Update current power state
    pub fn update_power_state(&self, state: PowerState);
    
    /// Queue message for delayed sending
    pub async fn queue_message(
        &self,
        destination: String,
        payload: Vec<u8>,
        priority: Priority
    ) -> Result<(), LowPowerError>;
    
    /// Send push notification
    pub async fn send_push_notification(
        &self,
        device_token: &str,
        message: &str
    ) -> Result<(), LowPowerError>;
    
    /// Optimize for battery level
    pub fn optimize_for_battery_level(&self, battery_level: u8) -> Result<(), LowPowerError>;
    
    /// Get power management statistics
    pub fn get_stats(&self) -> LowPowerStats;
    
    /// Start background power monitoring
    pub async fn start_monitoring(&self) -> Result<(), LowPowerError>;
}

#[derive(Debug, Clone)]
pub enum PowerState {
    ScreenOn,
    ScreenOff,
    PowerSaveMode,
    CriticalBattery,
}

#[derive(Debug)]
pub struct LowPowerStats {
    pub current_state: PowerState,
    pub battery_level: Option<u8>,
    pub cover_traffic_ratio: f32,
    pub queued_messages: usize,
    pub push_notifications_sent: u64,
    pub power_events: u64,
}
```

### Performance Optimizer

```rust
pub use nyx_core::performance::{
    PerformanceOptimizer,
    PerformanceConfig,
    PerformanceMetrics,
    BufferPool,
    OptimizationEvent,
    CleanupStats,
    PerformanceError,
};

impl PerformanceOptimizer {
    /// Create new performance optimizer
    pub fn new(config: PerformanceConfig) -> Self;
    
    /// Start optimization system
    pub async fn start(&self) -> Result<(), PerformanceError>;
    
    /// Get buffer from pool (zero-copy optimization)
    pub async fn get_buffer(&self) -> Vec<u8>;
    
    /// Return buffer to pool
    pub async fn return_buffer(&self, buffer: Vec<u8>);
    
    /// Record latency measurement
    pub async fn record_latency(&self, latency: Duration);
    
    /// Get current performance metrics
    pub async fn get_metrics(&self) -> PerformanceMetrics;
    
    /// Get buffer pool statistics
    pub async fn get_buffer_pool_stats(&self) -> BufferPoolStats;
    
    /// Force cleanup and garbage collection
    pub async fn force_cleanup(&self) -> Result<CleanupStats, PerformanceError>;
    
    /// Acquire thread pool permit
    pub async fn acquire_thread_permit(&self) -> Result<tokio::sync::SemaphorePermit<'_>, PerformanceError>;
}

#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub network_throughput: u64,
    pub active_connections: u32,
    pub packet_processing_rate: f32,
    pub error_rate: f32,
    pub latency_p50: Duration,
    pub latency_p95: Duration,
    pub latency_p99: Duration,
    pub gc_pressure: f32,
    pub thread_pool_utilization: f32,
    pub last_updated: Instant,
}
```

## Error Handling

### Error Types

```rust
/// Main error type for NyxNet
#[derive(Debug, thiserror::Error)]
pub enum NyxError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),
    
    #[error("Cryptographic error: {0}")]
    Crypto(#[from] CryptoError),
    
    #[error("Stream error: {0}")]
    Stream(#[from] StreamError),
    
    #[error("Routing error: {0}")]
    Routing(#[from] RoutingError),
    
    #[error("Low power error: {0}")]
    LowPower(#[from] LowPowerError),
    
    #[error("Performance error: {0}")]
    Performance(#[from] PerformanceError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Timeout")]
    Timeout,
    
    #[error("Connection closed")]
    ConnectionClosed,
    
    #[error("Invalid peer ID")]
    InvalidPeerId,
    
    #[error("Network unavailable")]
    NetworkUnavailable,
}

/// Transport-specific errors
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("Bind failed: {0}")]
    BindFailed(String),
    
    #[error("Send failed: {0}")]
    SendFailed(String),
    
    #[error("Receive failed: {0}")]
    ReceiveFailed(String),
    
    #[error("NAT traversal failed: {0}")]
    NatTraversalFailed(String),
    
    #[error("TCP fallback failed: {0}")]
    TcpFallbackFailed(String),
    
    #[error("Connection timeout")]
    ConnectionTimeout,
    
    #[error("Invalid address: {0}")]
    InvalidAddress(String),
}

// Additional error types defined in their respective modules...
```

### Error Context and Recovery

```rust
/// Result type alias for NyxNet operations
pub type NyxResult<T> = Result<T, NyxError>;

/// Error context for debugging
#[derive(Debug)]
pub struct ErrorContext {
    pub error: NyxError,
    pub timestamp: Instant,
    pub node_id: Option<String>,
    pub connection_id: Option<ConnectionId>,
    pub operation: String,
    pub retry_count: u32,
}

/// Error recovery strategies
pub trait ErrorRecovery {
    /// Determine if error is recoverable
    fn is_recoverable(&self) -> bool;
    
    /// Get suggested retry delay
    fn retry_delay(&self) -> Option<Duration>;
    
    /// Get maximum retry attempts
    fn max_retries(&self) -> u32;
    
    /// Attempt error recovery
    async fn recover(&self) -> NyxResult<()>;
}

impl ErrorRecovery for NyxError {
    fn is_recoverable(&self) -> bool {
        match self {
            NyxError::Timeout => true,
            NyxError::Transport(TransportError::ConnectionTimeout) => true,
            NyxError::NetworkUnavailable => true,
            NyxError::ConnectionClosed => true,
            _ => false,
        }
    }
    
    fn retry_delay(&self) -> Option<Duration> {
        match self {
            NyxError::Timeout => Some(Duration::from_millis(100)),
            NyxError::NetworkUnavailable => Some(Duration::from_secs(1)),
            _ => None,
        }
    }
    
    fn max_retries(&self) -> u32 {
        match self {
            NyxError::Timeout => 3,
            NyxError::NetworkUnavailable => 5,
            _ => 0,
        }
    }
    
    async fn recover(&self) -> NyxResult<()> {
        // Implementation depends on error type
        // May involve reconnection, path switching, etc.
        Ok(())
    }
}
```

## Examples

### Complete Node Setup

```rust
use nyx_core::*;
use nyx_daemon::*;
use nyx_client::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    // Create comprehensive configuration
    let mut config = NyxConfig::new();
    
    // Basic node settings
    config.node_id = "production-node-1".to_string();
    config.listen_port = 44380;
    config.max_connections = 1000;
    
    // Enable all advanced features
    config.multipath = MultipathConfig {
        enabled: true,
        max_paths: 4,
        algorithm: RoutingAlgorithm::Adaptive,
        path_probe_interval: Duration::from_secs(5),
        ..Default::default()
    };
    
    config.low_power = LowPowerConfig {
        enabled: true,
        screen_detection_enabled: true,
        cover_ratio: 0.1,
        battery_threshold: 15,
        ..Default::default()
    };
    
    config.tcp_fallback = TcpFallbackConfig {
        enabled: true,
        max_retries: 3,
        proxy_support: Some(ProxyConfig {
            proxy_type: ProxyType::Socks5,
            address: "proxy.example.com".to_string(),
            port: 1080,
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            connect_timeout: Duration::from_secs(10),
        }),
        ..Default::default()
    };
    
    config.performance = PerformanceConfig {
        enable_auto_tuning: true,
        max_cpu_threshold: 80.0,
        target_latency_p95: Duration::from_millis(50),
        enable_zero_copy: true,
        enable_batch_processing: true,
        batch_size: 32,
        ..Default::default()
    };
    
    // Create and start daemon
    let mut daemon = NyxDaemon::new(config).await?;
    daemon.start().await?;
    
    // Create client for sending messages
    let client_config = ClientConfig {
        node_id: "client-1".to_string(),
        bootstrap_nodes: vec!["127.0.0.1:44380".parse()?],
        ..Default::default()
    };
    
    let client = NyxClient::new(client_config).await?;
    
    // Example: Send a message with delivery confirmation
    let peer_id = NodeId::from_hex("abcdef...")?;
    let receipt = client.send_message_confirmed(
        &peer_id,
        b"Hello from NyxNet v1.0!",
        Duration::from_secs(30)
    ).await?;
    
    println!("Message delivered: {:?}", receipt);
    
    // Monitor performance
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            
            let stats = daemon.get_stats().await;
            println!("Node Stats: {:?}", stats);
            
            let client_stats = client.get_stats().await;
            println!("Client Stats: {:?}", client_stats);
        }
    });
    
    // Run until shutdown
    daemon.run().await?;
    
    Ok(())
}
```

### Mobile Client with Push Notifications

```rust
use nyx_client::*;
use nyx_core::mobile::*;

async fn setup_mobile_client() -> Result<NyxClient, NyxError> {
    let mut config = ClientConfig::default();
    
    // Mobile-optimized settings
    config.low_power = Some(LowPowerConfig {
        enabled: true,
        screen_detection_enabled: true,
        cover_ratio: 0.05, // Very low for battery savings
        battery_threshold: 20,
        push_config: Some(PushConfig {
            provider: PushProvider::FCM,
            fcm_server_key: "your-fcm-server-key".to_string(),
            device_token: "device-registration-token".to_string(),
            ..Default::default()
        }),
        ..Default::default()
    });
    
    // Prefer TCP fallback for mobile networks
    config.tcp_fallback = TcpFallbackConfig {
        enabled: true,
        prefer_tcp: true, // Mobile networks often have NAT issues
        ..Default::default()
    };
    
    let client = NyxClient::new(config).await?;
    
    // Register for push notifications
    client.enable_push_notifications().await?;
    
    Ok(client)
}
```

---

**Note**: This API documentation reflects the complete v1.0 implementation. For the most up-to-date information, please refer to the generated rustdoc documentation: `cargo doc --open`
