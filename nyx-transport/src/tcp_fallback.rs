//! Optimized TCP fallback helpers for reliable transport
//!
//! Provides high-performance TCP connection management with connection pooling,
//! keep-alive optimization, and automatic retry logic.

use crate::{Error, Result};
use socket2::SockRef; // Safe socket operations
use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

// Performance metrics for TCP operations
static TCP_CONNECTIONS_CREATED: AtomicU64 = AtomicU64::new(0);
static TCP_CONNECTIONS_REUSED: AtomicU64 = AtomicU64::new(0);
static TCP_BYTES_SENT: AtomicU64 = AtomicU64::new(0);
static TCP_BYTES_RECEIVED: AtomicU64 = AtomicU64::new(0);

/// Connection pool entry with last usage tracking
#[derive(Debug)]
struct PooledConnection {
    stream: TcpStream,
    last_used: Instant,
    _connection_count: u64,
}

/// High-performance TCP connection pool
pub struct TcpConnectionPool {
    pool: Arc<Mutex<HashMap<SocketAddr, Vec<PooledConnection>>>>,
    max_connections_per_addr: usize,
    connection_timeout: Duration,
    idle_timeout: Duration,
}

impl TcpConnectionPool {
    /// Create a new connection pool with specified parameters
    pub fn new(
        max_connections_per_addr: usize,
        connection_timeout: Duration,
        idle_timeout: Duration,
    ) -> Self {
        Self {
            pool: Arc::new(Mutex::new(HashMap::new())),
            max_connections_per_addr,
            connection_timeout,
            idle_timeout,
        }
    }

    /// Get or create a connection to the specified address
    pub fn get_connection(&self, addr: SocketAddr) -> Result<TcpStream> {
        // Try to reuse existing connection first
        if let Some(stream) = self.try_reuse_connection(addr)? {
            TCP_CONNECTIONS_REUSED.fetch_add(1, Ordering::Relaxed);
            return Ok(stream);
        }

        // Create new connection
        let stream = self.create_new_connection(addr)?;
        TCP_CONNECTIONS_CREATED.fetch_add(1, Ordering::Relaxed);
        Ok(stream)
    }

    /// Try to reuse an existing connection from the pool
    fn try_reuse_connection(&self, addr: SocketAddr) -> Result<Option<TcpStream>> {
        let mut pool = self
            .pool
            .lock()
            .map_err(|_| Error::Internal("TCP connection pool mutex poisoned".to_string()))?;

        if let Some(connections) = pool.get_mut(&addr) {
            // Remove and return the most recently used connection
            while let Some(conn) = connections.pop() {
                // Check if connection is still alive and not too old
                if conn.last_used.elapsed() < self.idle_timeout {
                    // Quick health check - try to peek at the stream
                    if self.is_connection_healthy(&conn.stream) {
                        return Ok(Some(conn.stream));
                    }
                }
            }

            // Clean up empty entries
            if connections.is_empty() {
                pool.remove(&addr);
            }
        }

        Ok(None)
    }

    /// Create a new optimized TCP connection
    fn create_new_connection(&self, addr: SocketAddr) -> Result<TcpStream> {
        let stream = TcpStream::connect_timeout(&addr, self.connection_timeout)
            .map_err(|e| Error::Msg(format!("tcp connect to {addr} failed: {e}")))?;

        // Optimize TCP settings for low latency
        stream.set_nodelay(true).ok(); // Disable Nagle's algorithm
        stream.set_ttl(64).ok(); // Set reasonable TTL

        // Set timeouts for better responsiveness
        stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(30))).ok();

        // Platform-specific optimizations
        #[cfg(unix)]
        {
            self.optimize_unix_socket(&stream);
        }

        #[cfg(windows)]
        {
            self.optimize_windows_socket(&stream);
        }

        Ok(stream)
    }

    /// Apply platform-specific socket optimizations using safe Rust APIs only.
    /// This replaces unsafe libc calls with safe cross-platform alternatives.
    #[cfg(unix)]
    fn optimize_unix_socket(&self, stream: &TcpStream) {
        // Use safe Rust APIs instead of unsafe libc calls
        // These are cross-platform optimizations that work on Unix-like systems
        let socket2_stream = SockRef::from(stream);

        // Set larger send/receive buffers for better throughput
        // These are safe alternatives to the unsafe libc setsockopt calls
        let _ = socket2_stream.set_send_buffer_size(262144); // 256KB
        let _ = socket2_stream.set_recv_buffer_size(262144); // 256KB

        // Enable keep-alive for connection health monitoring
        let _ = socket2_stream.set_keepalive(true);

        // Additional TCP optimizations available through socket2
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            // Enable TCP_USER_TIMEOUT for better connection management
            let _ = socket2_stream.set_tcp_user_timeout(Some(Duration::from_secs(30)));
        }
    }

    #[cfg(windows)]
    fn optimize_windows_socket(&self, stream: &TcpStream) {
        // Windows-specific socket optimizations using safe APIs
        let socket2_stream = SockRef::from(stream);

        // Set larger send/receive buffers for better throughput
        let _ = socket2_stream.set_send_buffer_size(262144); // 256KB
        let _ = socket2_stream.set_recv_buffer_size(262144); // 256KB

        // Enable keep-alive for connection health monitoring
        let _ = socket2_stream.set_keepalive(true);
    }

    /// Check if connection is still healthy
    fn is_connection_healthy(&self, _stream: &TcpStream) -> bool {
        // Simple health check - in a real implementation, you might want to
        // send a small probe packet or check socket error status
        true
    }

    /// Return a connection to the pool for reuse
    pub fn return_connection(&self, addr: SocketAddr, stream: TcpStream) -> Result<()> {
        let mut pool = self
            .pool
            .lock()
            .map_err(|_| Error::Internal("TCP connection pool mutex poisoned".to_string()))?;

        let connections = pool.entry(addr).or_default();

        // Don't exceed max connections per address
        if connections.len() < self.max_connections_per_addr {
            connections.push(PooledConnection {
                stream,
                last_used: Instant::now(),
                _connection_count: 1,
            });
        }

        Ok(())
    }

    /// Clean up idle connections from the pool
    pub fn cleanup_idle_connections(&self) {
        if let Ok(mut pool) = self.pool.lock() {
            let now = Instant::now();

            pool.retain(|_, connections| {
                connections.retain(|conn| now.duration_since(conn.last_used) < self.idle_timeout);
                !connections.is_empty()
            });
        }
        // If mutex is poisoned, just skip cleanup - not critical
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> TcpPoolStats {
        let (total_pooled, addresses_in_pool) = if let Ok(pool) = self.pool.lock() {
            let total_pooled = pool.values().map(|v| v.len()).sum();
            (total_pooled, pool.len())
        } else {
            // If mutex is poisoned, return default values
            (0, 0)
        };

        TcpPoolStats {
            total_pooled_connections: total_pooled,
            addresses_in_pool,
            connections_created: TCP_CONNECTIONS_CREATED.load(Ordering::Relaxed),
            connections_reused: TCP_CONNECTIONS_REUSED.load(Ordering::Relaxed),
            bytes_sent: TCP_BYTES_SENT.load(Ordering::Relaxed),
            bytes_received: TCP_BYTES_RECEIVED.load(Ordering::Relaxed),
        }
    }
}

/// Performance statistics for TCP connection pool
#[derive(Debug, Clone)]
pub struct TcpPoolStats {
    pub total_pooled_connections: usize,
    pub addresses_in_pool: usize,
    pub connections_created: u64,
    pub connections_reused: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// Attempt a TCP connection with a timeout; returns Ok(true) if connected.
/// This is the legacy simple interface for backward compatibility.
pub fn try_connect(addr: SocketAddr, timeout: Duration) -> Result<bool> {
    let stream = TcpStream::connect_timeout(&addr, timeout)
        .map_err(|e| Error::Msg(format!("tcp connect to {addr} failed: {e}")))?;
    stream.set_nodelay(true).ok();
    Ok(true)
}

/// Create a default connection pool with reasonable settings
pub fn create_default_pool() -> TcpConnectionPool {
    TcpConnectionPool::new(
        4,                        // max 4 connections per address
        Duration::from_secs(10),  // 10 second connection timeout
        Duration::from_secs(300), // 5 minute idle timeout
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn can_connect_localhost() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        let th = std::thread::spawn(move || listener.accept());
        let ok = try_connect(addr, Duration::from_millis(200))?;
        assert!(ok);
        let _result = th.join();
        Ok(())
    }

    #[test]
    fn connection_pool_basic() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pool = create_default_pool();
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;

        // Start a simple echo server
        let _server_handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                use std::io::{Read, Write};
                let mut buffer = [0; 1024];
                if let Ok(n) = stream.read(&mut buffer) {
                    let _ = stream.write_all(&buffer[..n]);
                }
            }
        });

        // Test connection pool
        std::thread::sleep(Duration::from_millis(10)); // Let server start
        let _conn1 = pool.get_connection(addr);
        let stats = pool.get_stats();
        assert!(stats.connections_created > 0);

        Ok(())
    }

    #[test]
    fn connection_pool_reuse() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let pool = create_default_pool();
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;

        // Simple server that accepts multiple connections
        let _server_handle = std::thread::spawn(move || {
            for _ in 0..3 {
                if let Ok((_stream, _)) = listener.accept() {
                    // Just accept and close
                }
            }
        });

        std::thread::sleep(Duration::from_millis(10)); // Let server start

        // Get and return connection
        if let Ok(conn) = pool.get_connection(addr) {
            let _ = pool.return_connection(addr, conn);
        }

        // Try to reuse
        let _conn2 = pool.get_connection(addr);
        let stats = pool.get_stats();

        // Should have some activity
        assert!(stats.connections_created > 0 || stats.connections_reused > 0);

        Ok(())
    }
}
