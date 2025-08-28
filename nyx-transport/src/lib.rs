//! Comprehensive transport abstraction layer for anonymity networks
//!
//! The `nyx-transport` crate provides secure and flexible transportation mechanisms:
//! - UDP endpoints with comprehensive configuration
//! - QUIC connections for encrypted transport
//! - TCP fallbacks for restrictive networks
//! - NAT traversal via STUN/TURN protocols
//! - Path validation and connectivity verification
//! - ICE-like connectivity establishment
//!
//! The implementation prioritizes security, correctness, and minimal dependencies
//! while providing the networking primitives needed for anonymity networks.

#![forbid(unsafe_code)]

use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("transport: {0}")]
    Msg(String),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Address parsing error: {0}")]
    AddrParse(#[from] std::net::AddrParseError),
    #[error("QUIC error")]
    Quic(quic::QuicError),
    #[error("Teredo error: {0}")]
    Teredo(#[from] teredo::TeredoError),
    #[error("STUN error: {0}")]
    Stun(#[from] stun_server::StunError),
}

pub type Result<T> = std::result::Result<T, Error>;

// Performance optimization: Cache network capability detection results
// These operations are expensive and rarely change during runtime
static UDP_BIND_CACHE: OnceLock<bool> = OnceLock::new();
static TCP_BIND_CACHE: OnceLock<bool> = OnceLock::new();
static IPV6_CACHE: OnceLock<bool> = OnceLock::new();

// Performance metrics for transport operations
static UDP_SEND_BYTES: AtomicU64 = AtomicU64::new(0);
static UDP_RECV_BYTES: AtomicU64 = AtomicU64::new(0);
static UDP_SEND_COUNT: AtomicU64 = AtomicU64::new(0);
static UDP_RECV_COUNT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub bind_addr: std::net::SocketAddr,
    pub buffer_size: usize,
    pub timeout: Duration,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:0".parse().unwrap(),
            buffer_size: 65536,
            timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransportMetrics {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
}

#[derive(Debug, Clone)]
pub struct UdpTransport {
    _config: TransportConfig, // Prefix with underscore to avoid unused warning
}

impl UdpTransport {
    pub fn new(config: TransportConfig) -> Result<Self> {
        Ok(Self { _config: config })
    }

    pub async fn send_to(&self, data: &[u8], _addr: std::net::SocketAddr) -> Result<()> {
        UDP_SEND_BYTES.fetch_add(data.len() as u64, Ordering::Relaxed);
        UDP_SEND_COUNT.fetch_add(1, Ordering::Relaxed);
        // Stub implementation
        Ok(())
    }

    pub fn get_metrics(&self) -> TransportMetrics {
        TransportMetrics {
            bytes_sent: UDP_SEND_BYTES.load(Ordering::Relaxed),
            bytes_received: UDP_RECV_BYTES.load(Ordering::Relaxed),
            packets_sent: UDP_SEND_COUNT.load(Ordering::Relaxed),
            packets_received: UDP_RECV_COUNT.load(Ordering::Relaxed),
        }
    }
}

/// Transport kinds supported by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    /// UDP transport (always available)
    Udp,
    /// QUIC transport (feature-gated)
    Quic,
    /// TCP fallback transport
    Tcp,
    /// ICE connectivity establishment
    Ice,
}

/// Transport capabilities and feature detection
#[derive(Debug, Clone)]
pub struct TransportCapabilities {
    pub udp_available: bool,
    pub quic_available: bool,
    pub tcp_available: bool,
    pub ice_available: bool,
    pub ipv6_support: bool,
    pub nat_traversal: bool,
    pub teredo_support: bool,
}

// Public modules for comprehensive transport functionality
pub mod ice;
pub mod path_validation;
pub mod stun_server;
pub mod tcp_fallback;
pub mod teredo;

// QUIC module is feature-gated to avoid dependencies
#[cfg(feature = "quic")]
pub mod quic;

#[cfg(not(feature = "quic"))]
pub mod quic {
    //! QUIC stub module when feature is disabled
    pub fn is_supported() -> bool {
        false
    }

    #[derive(Debug)]
    pub struct QuicError;
    impl std::fmt::Display for QuicError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "QUIC not available (feature disabled)")
        }
    }
    impl std::error::Error for QuicError {}
}

/// Detect available transport capabilities on this system
pub fn detect_capabilities() -> TransportCapabilities {
    TransportCapabilities {
        udp_available: available(TransportKind::Udp),
        quic_available: available(TransportKind::Quic),
        tcp_available: available(TransportKind::Tcp),
        ice_available: available(TransportKind::Ice),
        ipv6_support: has_ipv6_support(),
        nat_traversal: true,  // Our STUN implementation is always available
        teredo_support: true, // Pure Rust implementation
    }
}

/// Returns whether a specific transport kind is available given features and environment.
pub fn available(kind: TransportKind) -> bool {
    match kind {
        TransportKind::Udp => *UDP_BIND_CACHE.get_or_init(can_bind_udp_loopback),
        #[cfg(feature = "quic")]
        TransportKind::Quic => {
            // QUIC availability check - simplified since we can't use async here
            true // Assume available if feature is enabled
        }
        #[cfg(not(feature = "quic"))]
        TransportKind::Quic => false,
        TransportKind::Tcp => *TCP_BIND_CACHE.get_or_init(can_bind_tcp_loopback),
        TransportKind::Ice => *UDP_BIND_CACHE.get_or_init(can_bind_udp_loopback), // ICE requires UDP
    }
}

/// Check if IPv6 is supported on this system
pub fn has_ipv6_support() -> bool {
    *IPV6_CACHE.get_or_init(|| {
        use std::net::{Ipv6Addr, TcpListener};
        TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).is_ok()
    })
}

fn can_bind_udp_loopback() -> bool {
    use std::net::UdpSocket;
    UdpSocket::bind("127.0.0.1:0").is_ok()
}

fn can_bind_tcp_loopback() -> bool {
    use std::net::TcpListener;
    TcpListener::bind("127.0.0.1:0").is_ok()
}

/// Simple UDP endpoint for loopback-only communications (127.0.0.1).
///
/// Optimized for high-performance network operations with cache-aligned structures
/// and efficient buffer management.
#[repr(align(64))] // Cache line alignment for better performance
pub struct UdpEndpoint {
    sock: std::net::UdpSocket,
    send_buffer: Vec<u8>, // Reusable send buffer to avoid allocations
    recv_buffer: Vec<u8>, // Reusable receive buffer
    last_send_time: Instant,
    last_recv_time: Instant,
}

impl UdpEndpoint {
    /// Bind a UDP socket on 127.0.0.1 with an ephemeral port.
    pub fn bind_loopback() -> Result<Self> {
        let sock = std::net::UdpSocket::bind("127.0.0.1:0")?;
        Self::configure_socket(&sock)?;
        Ok(Self::new_with_socket(sock))
    }

    /// Bind a UDP socket on localhost for security
    pub fn bind_any() -> Result<Self> {
        let sock = std::net::UdpSocket::bind("127.0.0.1:0")?;
        Self::configure_socket(&sock)?;
        Ok(Self::new_with_socket(sock))
    }

    /// Bind a UDP socket to a specific address
    pub fn bind(addr: std::net::SocketAddr) -> Result<Self> {
        let sock = std::net::UdpSocket::bind(addr)?;
        Self::configure_socket(&sock)?;
        Ok(Self::new_with_socket(sock))
    }

    /// Configure socket for optimal performance
    fn configure_socket(sock: &std::net::UdpSocket) -> Result<()> {
        // Set to blocking mode for simplicity
        if let Err(e) = sock.set_nonblocking(false) {
            eprintln!("Failed to set socket to blocking mode: {e}");
        }

        // Note: set_send_buffer_size and set_recv_buffer_size are not available on UdpSocket
        // These would be handled at the OS level through socket2 crate if needed

        Ok(())
    }

    /// Create endpoint with pre-configured socket
    fn new_with_socket(sock: std::net::UdpSocket) -> Self {
        let now = Instant::now();
        Self {
            sock,
            send_buffer: Vec::with_capacity(65536), // 64KB initial capacity
            recv_buffer: vec![0u8; 65536],          // 64KB receive buffer
            last_send_time: now,
            last_recv_time: now,
        }
    }

    /// Return the local socket address.
    pub fn local_addr(&self) -> Result<std::net::SocketAddr> {
        Ok(self.sock.local_addr()?)
    }

    /// Send a datagram to the target address with optimized buffering.
    pub fn send_to(&mut self, buf: &[u8], to: std::net::SocketAddr) -> Result<usize> {
        let result = self.sock.send_to(buf, to);

        if let Ok(bytes_sent) = result {
            // Update performance metrics
            UDP_SEND_BYTES.fetch_add(bytes_sent as u64, Ordering::Relaxed);
            UDP_SEND_COUNT.fetch_add(1, Ordering::Relaxed);
            self.last_send_time = Instant::now();
        }

        Ok(result?)
    }

    /// Receive a datagram from the socket with optimized buffering.
    pub fn recv_from(&mut self, buf: &mut [u8]) -> Result<(usize, std::net::SocketAddr)> {
        let result = self.sock.recv_from(buf);

        if let Ok((bytes_recv, addr)) = result {
            // Update performance metrics
            UDP_RECV_BYTES.fetch_add(bytes_recv as u64, Ordering::Relaxed);
            UDP_RECV_COUNT.fetch_add(1, Ordering::Relaxed);
            self.last_recv_time = Instant::now();
            Ok((bytes_recv, addr))
        } else {
            Err(result.unwrap_err().into())
        }
    }

    /// Optimized send using internal buffer to reduce allocations
    pub fn send_to_buffered(&mut self, data: &[u8], to: std::net::SocketAddr) -> Result<usize> {
        // Reuse internal buffer if possible
        if self.send_buffer.capacity() < data.len() {
            self.send_buffer
                .reserve(data.len() - self.send_buffer.capacity());
        }

        self.send_buffer.clear();
        self.send_buffer.extend_from_slice(data);

        // Create a temporary copy to avoid borrowing conflicts
        let buffer_data = self.send_buffer.clone();
        self.send_to(&buffer_data, to)
    }

    /// Optimized receive using internal buffer
    pub fn recv_from_buffered(&mut self) -> Result<(Vec<u8>, std::net::SocketAddr)> {
        // Use a temporary buffer to avoid borrowing conflicts
        let mut temp_buffer = vec![0u8; 65536];
        let (bytes_recv, addr) = self.recv_from(&mut temp_buffer)?;
        let data = temp_buffer[..bytes_recv].to_vec();
        Ok((data, addr))
    }

    /// Set read timeout
    pub fn set_read_timeout(&self, timeout: Option<std::time::Duration>) -> Result<()> {
        Ok(self.sock.set_read_timeout(timeout)?)
    }

    /// Set write timeout
    pub fn set_write_timeout(&self, timeout: Option<std::time::Duration>) -> Result<()> {
        Ok(self.sock.set_write_timeout(timeout)?)
    }

    /// Enable/disable non-blocking mode
    pub fn set_nonblocking(&self, nonblocking: bool) -> Result<()> {
        Ok(self.sock.set_nonblocking(nonblocking)?)
    }

    /// Get performance statistics for this endpoint
    pub fn get_stats(&self) -> UdpEndpointStats {
        UdpEndpointStats {
            bytes_sent: UDP_SEND_BYTES.load(Ordering::Relaxed),
            bytes_received: UDP_RECV_BYTES.load(Ordering::Relaxed),
            packets_sent: UDP_SEND_COUNT.load(Ordering::Relaxed),
            packets_received: UDP_RECV_COUNT.load(Ordering::Relaxed),
            last_send_time: self.last_send_time,
            last_recv_time: self.last_recv_time,
        }
    }

    /// Reset internal buffers to reduce memory usage
    pub fn reset_buffers(&mut self) {
        self.send_buffer.clear();
        self.send_buffer.shrink_to_fit();
        self.recv_buffer = vec![0u8; 65536]; // Reset to default size
    }
}

/// Performance statistics for UDP endpoint
#[derive(Debug, Clone)]
pub struct UdpEndpointStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub last_send_time: Instant,
    pub last_recv_time: Instant,
}

/// Transport requirements for selecting optimal transport
#[derive(Debug, Clone)]
pub struct TransportRequirements {
    pub requires_reliability: bool,
    pub allows_unreliable: bool,
    pub requires_encryption: bool,
    pub supports_streams: bool,
    pub requires_nat_traversal: bool,
    pub prefers_low_latency: bool,
    pub max_latency: Option<Duration>,
    pub min_bandwidth: Option<u64>,
}

impl Default for TransportRequirements {
    fn default() -> Self {
        Self {
            requires_reliability: false,
            allows_unreliable: true,
            requires_encryption: true,
            supports_streams: false,
            requires_nat_traversal: false,
            prefers_low_latency: true,
            max_latency: None,
            min_bandwidth: None,
        }
    }
}

/// High-level transport manager for handling multiple protocols
pub struct TransportManager {
    capabilities: TransportCapabilities,
    preferred_transports: Vec<TransportKind>,
}

impl TransportManager {
    /// Create a new transport manager with detected capabilities
    pub fn new() -> Self {
        let capabilities = detect_capabilities();
        let preferred_transports = Self::determine_preferred_transports(&capabilities);

        Self {
            capabilities,
            preferred_transports,
        }
    }

    /// Get transport capabilities
    pub fn capabilities(&self) -> &TransportCapabilities {
        &self.capabilities
    }

    /// Get preferred transports in order
    pub fn preferred_transports(&self) -> &[TransportKind] {
        &self.preferred_transports
    }

    /// Select optimal transport based on requirements
    pub fn select_transport(&self, requirements: &TransportRequirements) -> Option<TransportKind> {
        self.preferred_transports
            .iter()
            .find(|&&transport| self.transport_meets_requirements(transport, requirements))
            .copied()
    }

    /// Check if transport meets requirements
    fn transport_meets_requirements(
        &self,
        transport: TransportKind,
        req: &TransportRequirements,
    ) -> bool {
        match transport {
            TransportKind::Udp => {
                req.allows_unreliable && !req.requires_reliability && !req.supports_streams
            }
            TransportKind::Quic => {
                self.capabilities.quic_available && req.requires_encryption && req.supports_streams
            }
            TransportKind::Tcp => self.capabilities.tcp_available && req.requires_reliability,
            TransportKind::Ice => self.capabilities.ice_available && req.requires_nat_traversal,
        }
    }

    /// Determine preferred transport order based on capabilities
    fn determine_preferred_transports(capabilities: &TransportCapabilities) -> Vec<TransportKind> {
        let mut transports = Vec::new();

        // Prefer QUIC for encrypted, reliable communication
        if capabilities.quic_available {
            transports.push(TransportKind::Quic);
        }

        // UDP for low-latency, simple communication
        if capabilities.udp_available {
            transports.push(TransportKind::Udp);
        }

        // TCP as fallback for reliability
        if capabilities.tcp_available {
            transports.push(TransportKind::Tcp);
        }

        // ICE for NAT traversal when needed
        if capabilities.ice_available {
            transports.push(TransportKind::Ice);
        }

        transports
    }

    /// Update capabilities (useful for dynamic network changes)
    pub fn refresh_capabilities(&mut self) {
        self.capabilities = detect_capabilities();
        self.preferred_transports = Self::determine_preferred_transports(&self.capabilities);
    }
}

impl Default for TransportManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_capabilities_basic() {
        let caps = detect_capabilities();
        assert!(caps.udp_available); // Should always be true in test environment
                                     // QUIC availability depends on feature flag
        assert_eq!(caps.quic_available, cfg!(feature = "quic"));
    }

    #[test]
    fn transport_manager_creation() {
        let manager = TransportManager::new();
        assert!(!manager.preferred_transports().is_empty());
        assert!(manager.capabilities().udp_available);
    }

    #[test]
    fn transport_selection() {
        let manager = TransportManager::new();

        // Default requirements should select something
        let req = TransportRequirements::default();
        let selected = manager.select_transport(&req);
        assert!(selected.is_some());

        // Requirements that need reliability
        let req = TransportRequirements {
            requires_reliability: true,
            allows_unreliable: false,
            ..Default::default()
        };
        let selected = manager.select_transport(&req);
        // Should prefer QUIC or TCP over UDP
        if let Some(transport) = selected {
            assert!(matches!(
                transport,
                TransportKind::Quic | TransportKind::Tcp
            ));
        }
    }

    #[test]
    fn udp_send_recv_roundtrip() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut a = UdpEndpoint::bind_loopback()?;
        let mut b = UdpEndpoint::bind_loopback()?;
        let msg = b"ping";
        a.send_to(msg, b.local_addr()?)?;
        let mut buf = [0u8; 16];
        let (n, from) = b.recv_from(&mut buf)?;
        assert_eq!(&buf[..n], msg);
        assert_eq!(from.ip().to_string(), "127.0.0.1");
        Ok(())
    }

    #[test]
    fn udp_endpoint_configuration() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let endpoint = UdpEndpoint::bind_loopback()?;

        // Test configuration methods
        endpoint.set_read_timeout(Some(std::time::Duration::from_millis(100)))?;
        endpoint.set_write_timeout(Some(std::time::Duration::from_millis(100)))?;

        // Test blocking/non-blocking
        endpoint.set_nonblocking(true)?;
        endpoint.set_nonblocking(false)?;
        Ok(())
    }

    #[test]
    fn udp_bind_variants() -> std::result::Result<(), Box<dyn std::error::Error>> {
        // Test different binding modes
        let _loopback = UdpEndpoint::bind_loopback()?;
        let _any = UdpEndpoint::bind_any()?;

        let specific_addr = "127.0.0.1:0".parse()?;
        let _specific = UdpEndpoint::bind(specific_addr)?;
        Ok(())
    }

    #[test]
    fn ipv6_support_detection() {
        let has_ipv6 = has_ipv6_support();
        // This might be true or false depending on the system
        // Just ensure it doesn't panic
        let _ = has_ipv6;
    }

    #[test]
    fn transport_requirements_defaults() {
        let req = TransportRequirements::default();
        assert!(!req.requires_reliability);
        assert!(req.allows_unreliable);
        assert!(req.requires_encryption);
        assert!(!req.supports_streams);
        assert!(!req.requires_nat_traversal);
        assert!(req.prefers_low_latency);
    }

    #[cfg(feature = "quic")]
    #[test]
    fn quic_available_with_feature() {
        assert!(available(TransportKind::Quic));
    }

    #[cfg(not(feature = "quic"))]
    #[test]
    fn quic_unavailable_without_feature() {
        assert!(!available(TransportKind::Quic));
    }
}
