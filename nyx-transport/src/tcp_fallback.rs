//! TCP Fallback Encapsulation
//!
//! When UDP traversal fails (e.g., restrictive NAT/Firewall) Nyx can tunnel
//! its fixed-size datagrams over a single TCP connection. Each Nyx packet is
//! length-prefixed with a 2-byte big-endian size (<= 1500). This framing keeps
//! boundaries so upper layers remain unchanged.
//!
//! The fallback layer intentionally keeps logic minimal: congestion control
//! becomes TCP's responsibility, and latency cost is accepted only when UDP is
//! unavailable.
//!
//! # Example
//! ```rust,ignore
//! // server
//! let srv = TcpEncapListener::bind(44380).await?;
//! // client
//! let conn = TcpEncapConnection::connect("example.com:44380").await?;
//! conn.send(&bytes).await?;
//! ```

#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::{net::{TcpListener, TcpStream}, io::{AsyncReadExt, AsyncWriteExt}, sync::{mpsc, RwLock, Mutex}};
use tracing::{info, error, warn, debug, instrument, Instrument};

const MAX_FRAME: usize = 2048; // generous upper bound; Nyx uses 1280

/// Enhanced TCP fallback configuration for production environments
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
    pub enable_compression: bool,
    pub proxy_support: Option<ProxyConfig>,
}

impl Default for TcpFallbackConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            connect_timeout: Duration::from_secs(30),
            keepalive_interval: Duration::from_secs(30),
            max_idle_time: Duration::from_secs(300),
            buffer_size: 8192,
            max_connections: 1000,
            retry_attempts: 3,
            retry_backoff: Duration::from_secs(1),
            enable_compression: false,
            proxy_support: None,
        }
    }
}

/// Proxy configuration for restrictive network environments
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

/// Connection statistics for monitoring
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub peer_addr: SocketAddr,
    pub connected_at: Instant,
    pub last_activity: Instant,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub connection_errors: u32,
    pub is_active: bool,
}

/// Length-prefixed read helper.
#[instrument(name = "tcp_read_frame", skip(stream))]
async fn read_frame(stream: &mut TcpStream) -> std::io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 2];
    match stream.read_exact(&mut len_buf).await {
        Ok(_) => {
            let len = u16::from_be_bytes(len_buf) as usize;
            if len == 0 || len > MAX_FRAME { return Err(std::io::ErrorKind::InvalidData.into()); }
            let mut data = vec![0u8; len];
            stream.read_exact(&mut data).await?;
            Ok(Some(data))
        }
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
        Err(e) => Err(e),
    }
}

#[instrument(name = "tcp_write_frame", skip(stream, data), fields(bytes = data.len()))]
async fn write_frame(stream: &mut TcpStream, data: &[u8]) -> std::io::Result<()> {
    if data.len() > MAX_FRAME { return Err(std::io::ErrorKind::InvalidInput.into()); }
    stream.write_all(&(data.len() as u16).to_be_bytes()).await?;
    stream.write_all(data).await?;
    stream.flush().await
}

/// Server-side listener accepting encapsulated TCP connections.
pub struct TcpEncapListener {
    pub incoming: mpsc::Receiver<(SocketAddr, Vec<u8>)>,
    config: TcpFallbackConfig,
    connections: Arc<RwLock<HashMap<SocketAddr, Arc<Mutex<ConnectionStats>>>>>,
    connection_pool: Arc<RwLock<ConnectionPool>>,
}

/// Connection pool for efficient connection reuse
struct ConnectionPool {
    idle_connections: HashMap<SocketAddr, Vec<TcpStream>>,
    max_idle_per_peer: usize,
    cleanup_interval: Duration,
}

impl ConnectionPool {
    fn new() -> Self {
        Self {
            idle_connections: HashMap::new(),
            max_idle_per_peer: 5,
            cleanup_interval: Duration::from_secs(60),
        }
    }

    async fn get_connection(&mut self, peer: &SocketAddr) -> Option<TcpStream> {
        if let Some(connections) = self.idle_connections.get_mut(peer) {
            connections.pop()
        } else {
            None
        }
    }

    async fn return_connection(&mut self, peer: SocketAddr, stream: TcpStream) {
        let connections = self.idle_connections.entry(peer).or_insert_with(Vec::new);
        if connections.len() < self.max_idle_per_peer {
            connections.push(stream);
        }
    }

    async fn cleanup_idle(&mut self) {
        // Remove idle connections that have been unused for too long
        self.idle_connections.retain(|_, connections| {
            connections.retain(|_| true); // In a real implementation, check connection age
            !connections.is_empty()
        });
    }
}

impl TcpEncapListener {
    #[instrument(name="tcp_listener_bind", skip(port), fields(local_port = port))]
    pub async fn bind(port: u16) -> std::io::Result<Self> {
        Self::bind_with_config(port, TcpFallbackConfig::default()).await
    }

    #[instrument(name="tcp_listener_bind_config", skip(port, config), fields(local_port = port))]
    pub async fn bind_with_config(port: u16, config: TcpFallbackConfig) -> std::io::Result<Self> {
        let public = std::env::var("NYX_TCP_PUBLIC").ok().map(|v| v=="1" || v.eq_ignore_ascii_case("true")).unwrap_or(false);
        let bind_addr = if public { ("0.0.0.0", port) } else { ("127.0.0.1", port) };
        let listener = TcpListener::bind(bind_addr).await?;
        let (tx, rx) = mpsc::channel::<(SocketAddr, Vec<u8>)>(config.buffer_size);
        let connections = Arc::new(RwLock::new(HashMap::new()));
        let connection_pool = Arc::new(RwLock::new(ConnectionPool::new()));
        
        let connections_for_accept = Arc::clone(&connections);
        let config_for_accept = config.clone();
        
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut stream, addr)) => {
                        // Check connection limits
                        if connections_for_accept.read().await.len() >= config_for_accept.max_connections {
                            warn!("Maximum connections reached, rejecting {}", addr);
                            continue;
                        }

                        // Configure socket options. Tokio TcpStream exposes set_nodelay; keepalive configuration
                        // is platform-specific and not available uniformly on Tokio's wrapper, so we skip it here.
                        let _ = stream.set_nodelay(true);
                        
                        info!("tcp_fallback: connection from {}", addr);
                        
                        // Create connection stats
                        let stats = Arc::new(Mutex::new(ConnectionStats {
                            peer_addr: addr,
                            connected_at: Instant::now(),
                            last_activity: Instant::now(),
                            bytes_sent: 0,
                            bytes_received: 0,
                            packets_sent: 0,
                            packets_received: 0,
                            connection_errors: 0,
                            is_active: true,
                        }));
                        
                        connections_for_accept.write().await.insert(addr, stats.clone());
                        
                        let tx_clone = tx.clone();
                        let stats_clone = stats.clone();
                        let config_inner = config_for_accept.clone();
                        let connections_for_task = Arc::clone(&connections_for_accept);
                        
                        tokio::spawn(async move {
                            let mut last_keepalive = Instant::now();
                            
                            loop {
                                // Check for keepalive timeout
                                if last_keepalive.elapsed() > config_inner.max_idle_time {
                                    debug!("Connection {} idle timeout", addr);
                                    break;
                                }
                                
                                // Set read timeout to prevent hanging
                                let read_result = tokio::time::timeout(
                                    Duration::from_secs(1),
                                    read_frame(&mut stream)
                                ).await;
                                
                                match read_result {
                                    Ok(Ok(Some(packet))) => {
                                        // Update statistics
                                        {
                                            let mut stats_guard = stats_clone.lock().await;
                                            stats_guard.last_activity = Instant::now();
                                            stats_guard.bytes_received += packet.len() as u64;
                                            stats_guard.packets_received += 1;
                                        }
                                        
                                        last_keepalive = Instant::now();
                                        
                                        // Forward packet
                                        if tx_clone.send((addr, packet)).await.is_err() {
                                            debug!("Channel closed, terminating connection {}", addr);
                                            break;
                                        }
                                    }
                                    Ok(Ok(None)) => {
                                        debug!("Connection {} closed by peer", addr);
                                        break;
                                    }
                                    Ok(Err(e)) => {
                                        error!("tcp_fallback recv error from {}: {}", addr, e);
                                        stats_clone.lock().await.connection_errors += 1;
                                        break;
                                    }
                                    Err(_) => {
                                        // Timeout - continue loop for keepalive check
                                        continue;
                                    }
                                }
                            }
                            
                            // Mark connection as inactive
                            if let Some(stats) = connections_for_task.read().await.get(&addr) {
                                stats.lock().await.is_active = false;
                            }
                            
                            // Clean up connection
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            connections_for_task.write().await.remove(&addr);
                            debug!("Cleaned up connection {}", addr);
                            
                        }.instrument(tracing::info_span!("tcp_recv_loop", peer=%addr)));
                    }
                    Err(e) => error!("tcp_fallback accept error: {}", e),
                }
            }
        }.instrument(tracing::info_span!("tcp_accept_loop")));
        
        // Spawn cleanup task
        let connections_cleanup = Arc::clone(&connections);
        let cleanup_interval = config.max_idle_time;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                interval.tick().await;
                
                let mut to_remove = Vec::new();
                {
                    let connections_read = connections_cleanup.read().await;
                    for (addr, stats) in connections_read.iter() {
                        let stats_guard = stats.lock().await;
                        if !stats_guard.is_active || 
                           stats_guard.last_activity.elapsed() > cleanup_interval {
                            to_remove.push(*addr);
                        }
                    }
                }
                
                if !to_remove.is_empty() {
                    let mut connections_write = connections_cleanup.write().await;
                    for addr in to_remove {
                        connections_write.remove(&addr);
                        debug!("Removed stale connection {}", addr);
                    }
                }
            }
        }.instrument(tracing::info_span!("tcp_cleanup_task")));
        
        Ok(Self { 
            incoming: rx, 
            config, 
            connections,
            connection_pool,
        })
    }

    /// Get current connection statistics
    pub async fn get_connection_stats(&self) -> Vec<ConnectionStats> {
        let connections = self.connections.read().await;
        let mut stats = Vec::new();
        
        for (_, conn_stats) in connections.iter() {
            stats.push(conn_stats.lock().await.clone());
        }
        
        stats
    }

    /// Get total number of active connections
    pub async fn active_connections(&self) -> usize {
        self.connections.read().await.len()
    }
}

/// Client/peer connection over TCP encapsulation.
#[derive(Clone)]
pub struct TcpEncapConnection {
    stream: Arc<Mutex<TcpStream>>,
    peer: SocketAddr,
    config: TcpFallbackConfig,
    stats: Arc<Mutex<ConnectionStats>>,
}

impl TcpEncapConnection {
    #[instrument(name="tcp_client_connect", skip(addr), fields(server=%addr))]
    pub async fn connect(addr: &str) -> std::io::Result<Self> {
        Self::connect_with_config(addr, TcpFallbackConfig::default()).await
    }

    #[instrument(name="tcp_client_connect_config", skip(addr, config), fields(server=%addr))]
    pub async fn connect_with_config(addr: &str, config: TcpFallbackConfig) -> std::io::Result<Self> {
        let connect_future = TcpStream::connect(addr);
        
        let stream = match tokio::time::timeout(config.connect_timeout, connect_future).await {
            Ok(stream) => stream?,
            Err(_) => return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Connection timeout"
            )),
        };
        
        let _ = stream.set_nodelay(true);
        
        let peer = stream.peer_addr()?;
        info!("tcp_fallback: connected to {}", peer);
        
        let stats = Arc::new(Mutex::new(ConnectionStats {
            peer_addr: peer,
            connected_at: Instant::now(),
            last_activity: Instant::now(),
            bytes_sent: 0,
            bytes_received: 0,
            packets_sent: 0,
            packets_received: 0,
            connection_errors: 0,
            is_active: true,
        }));
        
        Ok(Self { 
            stream: Arc::new(Mutex::new(stream)), 
            peer, 
            config,
            stats,
        })
    }

    #[instrument(name="tcp_client_connect_with_retry", skip(addr, config), fields(server=%addr))]
    pub async fn connect_with_retry(addr: &str, config: TcpFallbackConfig) -> std::io::Result<Self> {
        let mut last_error = None;
        
        for attempt in 0..config.retry_attempts {
            match Self::connect_with_config(addr, config.clone()).await {
                Ok(conn) => {
                    if attempt > 0 {
                        info!("tcp_fallback: connected to {} after {} retries", addr, attempt);
                    }
                    return Ok(conn);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < config.retry_attempts - 1 {
                        let backoff = config.retry_backoff * (attempt + 1);
                        warn!("tcp_fallback: connection attempt {} failed, retrying in {:?}", 
                              attempt + 1, backoff);
                        tokio::time::sleep(backoff).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| std::io::Error::new(
            std::io::ErrorKind::ConnectionRefused,
            "All connection attempts failed"
        )))
    }

    #[instrument(name="tcp_client_send", skip(self, data), fields(bytes=data.len()))]
    pub async fn send(&self, data: &[u8]) -> std::io::Result<()> {
        let mut guard = self.stream.lock().await;
        let result = write_frame(&mut *guard, data).await;
        
        // Update statistics
        {
            let mut stats_guard = self.stats.lock().await;
            stats_guard.last_activity = Instant::now();
            if result.is_ok() {
                stats_guard.bytes_sent += data.len() as u64;
                stats_guard.packets_sent += 1;
            } else {
                stats_guard.connection_errors += 1;
            }
        }
        
        result
    }

    #[instrument(name="tcp_client_recv", skip(self))]
    pub async fn recv(&self) -> std::io::Result<Option<Vec<u8>>> {
        let mut guard = self.stream.lock().await;
        let result = read_frame(&mut *guard).await;
        
        // Update statistics
        {
            let mut stats_guard = self.stats.lock().await;
            stats_guard.last_activity = Instant::now();
            match &result {
                Ok(Some(data)) => {
                    stats_guard.bytes_received += data.len() as u64;
                    stats_guard.packets_received += 1;
                }
                Ok(None) => {
                    stats_guard.is_active = false;
                }
                Err(_) => {
                    stats_guard.connection_errors += 1;
                }
            }
        }
        
        result
    }

    #[instrument(name="tcp_client_recv_timeout", skip(self))]
    pub async fn recv_with_timeout(&self, timeout: Duration) -> std::io::Result<Option<Vec<u8>>> {
        match tokio::time::timeout(timeout, self.recv()).await {
            Ok(result) => result,
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Receive timeout"
            )),
        }
    }

    #[instrument(name="tcp_client_send_with_retry", skip(self, data), fields(bytes=data.len()))]
    pub async fn send_with_retry(&self, data: &[u8]) -> std::io::Result<()> {
        for attempt in 0..self.config.retry_attempts {
            match self.send(data).await {
                Ok(()) => return Ok(()),
                Err(e) if attempt < self.config.retry_attempts - 1 => {
                    warn!("tcp_fallback: send attempt {} failed: {}, retrying", attempt + 1, e);
                    tokio::time::sleep(self.config.retry_backoff).await;
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!()
    }

    /// Check if connection is still active
    pub async fn is_active(&self) -> bool {
        self.stats.lock().await.is_active
    }

    /// Get connection statistics
    pub async fn get_stats(&self) -> ConnectionStats {
        self.stats.lock().await.clone()
    }

    #[must_use] pub fn peer_addr(&self) -> SocketAddr { self.peer }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_roundtrip() {
        // Initialize tracing subscriber if available in the test environment.
        #[allow(unused_must_use)]
        {
            #[cfg(feature = "test_tracing")]
            {
                let _ = tracing_subscriber::fmt::try_init();
            }
        }
        let mut listener = TcpEncapListener::bind(4480).await.unwrap();
        let conn = TcpEncapConnection::connect("127.0.0.1:4480").await.unwrap();
        conn.send(&[1,2,3]).await.unwrap();
        
        // First packet
        if let Some((_, pkt)) = listener.incoming.recv().await {
            assert_eq!(pkt, vec![1,2,3]);
        } else { 
            panic!("no packet"); 
        }

        // Wait to ensure keepalive prevents closure and NAT idle drop.
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Second packet after idle period.
        conn.send(&[9,8,7]).await.unwrap();
        if let Some((_, pkt)) = listener.incoming.recv().await {
            assert_eq!(pkt, vec![9,8,7]);
        } else { 
            panic!("no packet 2"); 
        }
    }

    #[tokio::test]
    async fn test_connection_with_config() {
        let config = TcpFallbackConfig {
            enabled: true,
            connect_timeout: Duration::from_secs(5),
            keepalive_interval: Duration::from_secs(10),
            max_idle_time: Duration::from_secs(60),
            buffer_size: 4096,
            max_connections: 100,
            retry_attempts: 2,
            retry_backoff: Duration::from_millis(500),
            enable_compression: false,
            proxy_support: None,
        };

        let mut listener = TcpEncapListener::bind_with_config(4481, config.clone()).await.unwrap();
        let conn = TcpEncapConnection::connect_with_config("127.0.0.1:4481", config).await.unwrap();
        
        assert!(conn.is_active().await);
        
        conn.send(&[42]).await.unwrap();
        if let Some((addr, pkt)) = listener.incoming.recv().await {
            assert_eq!(pkt, vec![42]);
            // Client remote address is ephemeral; ensure server peer addr is the listener port
            assert_eq!(conn.peer_addr().port(), 4481);
            assert_ne!(addr.port(), 0);
        }
    }

    #[tokio::test]
    async fn test_connection_stats() {
        let mut listener = TcpEncapListener::bind(4482).await.unwrap();
        let conn = TcpEncapConnection::connect("127.0.0.1:4482").await.unwrap();
        
        let initial_stats = conn.get_stats().await;
        assert_eq!(initial_stats.packets_sent, 0);
        assert_eq!(initial_stats.packets_received, 0);
        
        conn.send(&[1, 2, 3, 4, 5]).await.unwrap();
        
        let updated_stats = conn.get_stats().await;
        assert_eq!(updated_stats.packets_sent, 1);
        assert_eq!(updated_stats.bytes_sent, 5);
    }

    #[tokio::test]
    async fn test_connection_timeout() {
        let config = TcpFallbackConfig {
            connect_timeout: Duration::from_millis(100),
            ..Default::default()
        };

        // Try to connect to a non-existent service
        let result = TcpEncapConnection::connect_with_config("127.0.0.1:9999", config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retry_mechanism() {
        let config = TcpFallbackConfig {
            connect_timeout: Duration::from_millis(100),
            retry_attempts: 3,
            retry_backoff: Duration::from_millis(50),
            ..Default::default()
        };

        let start = Instant::now();
        let result = TcpEncapConnection::connect_with_retry("127.0.0.1:9998", config).await;
        let elapsed = start.elapsed();
        
        assert!(result.is_err());
        // Should have taken at least 50ms * 2 retries = 100ms for backoff
        assert!(elapsed >= Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_listener_connection_limits() {
        let config = TcpFallbackConfig {
            max_connections: 2,
            ..Default::default()
        };

        let listener = TcpEncapListener::bind_with_config(4483, config).await.unwrap();
        
        // Should start with 0 connections
        assert_eq!(listener.active_connections().await, 0);
        
        let _conn1 = TcpEncapConnection::connect("127.0.0.1:4483").await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await; // Allow connection to be registered
        
        let _conn2 = TcpEncapConnection::connect("127.0.0.1:4483").await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await; // Allow connection to be registered
        
        // Should have 2 active connections
        assert_eq!(listener.active_connections().await, 2);
    }

    #[tokio::test]
    async fn test_frame_size_limits() {
        let listener = TcpEncapListener::bind(4484).await.unwrap();
        let conn = TcpEncapConnection::connect("127.0.0.1:4484").await.unwrap();
        
        // Test maximum frame size
        let large_data = vec![0u8; MAX_FRAME];
        let result = conn.send(&large_data).await;
        assert!(result.is_ok());
        
        // Test oversized frame (should fail)
        let oversized_data = vec![0u8; MAX_FRAME + 1];
        let result = conn.send(&oversized_data).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bidirectional_communication() {
        let mut listener = TcpEncapListener::bind(4485).await.unwrap();
        let conn = TcpEncapConnection::connect("127.0.0.1:4485").await.unwrap();
        
        // Send from client to server
        conn.send(&[1, 2, 3]).await.unwrap();
        
        if let Some((_, pkt)) = listener.incoming.recv().await {
            assert_eq!(pkt, vec![1, 2, 3]);
        }
        
        // Verify statistics are updated on both sides
        let conn_stats = conn.get_stats().await;
        assert_eq!(conn_stats.packets_sent, 1);
        assert_eq!(conn_stats.bytes_sent, 3);
        
        let listener_stats = listener.get_connection_stats().await;
        assert_eq!(listener_stats.len(), 1);
        assert_eq!(listener_stats[0].packets_received, 1);
        assert_eq!(listener_stats[0].bytes_received, 3);
    }
} 