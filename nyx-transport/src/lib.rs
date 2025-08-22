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

use thiserror::Error;
use std::sync::OnceLock;

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
        nat_traversal: true,   // Our STUN implementation is always available
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
pub struct UdpEndpoint {
    sock: std::net::UdpSocket,
}

impl UdpEndpoint {
    /// Bind a UDP socket on 127.0.0.1 with an ephemeral port.
    pub fn bind_loopback() -> Result<Self> {
        let sock = std::net::UdpSocket::bind("127.0.0.1:0")?;
        if let Err(e) = sock.set_nonblocking(false) {
            tracing::warn!("Failed to set socket to blocking mode: {}", e);
        }
        Ok(Self { sock })
    }

    /// Bind a UDP socket on localhost for security
    pub fn bind_any() -> Result<Self> {
        let sock = std::net::UdpSocket::bind("127.0.0.1:0")?;
        if let Err(e) = sock.set_nonblocking(false) {
            tracing::warn!("Failed to set socket to blocking mode: {}", e);
        }
        Ok(Self { sock })
    }

    /// Bind a UDP socket to a specific address
    pub fn bind(addr: std::net::SocketAddr) -> Result<Self> {
        let sock = std::net::UdpSocket::bind(addr)?;
        if let Err(e) = sock.set_nonblocking(false) {
            tracing::warn!("Failed to set socket to blocking mode: {}", e);
        }
        Ok(Self { sock })
    }

    /// Return the local socket address.
    pub fn local_addr(&self) -> Result<std::net::SocketAddr> {
        Ok(self.sock.local_addr()?)
    }

    /// Send a datagram to the target address.
    pub fn send_to(&self, buf: &[u8], to: std::net::SocketAddr) -> Result<usize> {
        Ok(self.sock.send_to(buf, to)?)
    }

    /// Receive a datagram from the socket.
    pub fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, std::net::SocketAddr)> {
        Ok(self.sock.recv_from(buf)?)
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
    pub fn setnonblocking(&self, nonblocking: bool) -> Result<()> {
        Ok(self.sock.set_nonblocking(nonblocking)?)
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

    /// Get the detected transport capabilities
    pub fn capabilities(&self) -> &TransportCapabilities {
        &self.capabilities
    }

    /// Get the preferred transport order
    pub fn preferred_transports(&self) -> &[TransportKind] {
        &self.preferred_transports
    }

    /// Determine the best available transport for a given scenario
    pub fn select_transport(&self, requirements: &TransportRequirements) -> Option<TransportKind> {
        for &transport in self.preferred_transports() {
            if self.transport_meets_requirements(transport, requirements) {
                return Some(transport);
            }
        }
        None
    }

    /// Determine the best available transport for a given scenario (optimized version)
    /// This method is optimized for performance by avoiding unnecessary work
    #[inline(always)]
    pub fn select_transport_optimized(&self, requirements: &TransportRequirements) -> Option<TransportKind> {
        // Fast path: check most likely candidates first based on requirements
        if requirements.requires_reliability {
            if requirements.supports_streams && self.capabilities.quic_available {
                return Some(TransportKind::Quic);
            }
            if self.capabilities.tcp_available {
                return Some(TransportKind::Tcp);
            }
        } else if requirements.requires_nat_traversal && self.capabilities.ice_available {
            return Some(TransportKind::Ice);
        } else if self.capabilities.udp_available {
            return Some(TransportKind::Udp);
        }

        // Fallback to original linear search
        self.select_transport(requirements)
    }

    /// Check if a transport meets the given requirements
    fn transport_meets_requirements(
        &self,
        transport: TransportKind,
        req: &TransportRequirements,
    ) -> bool {
        match transport {
            TransportKind::Udp => {
                self.capabilities().udp_available
                    && (!req.requires_reliability || req.allows_unreliable)
            }
            TransportKind::Quic => {
                self.capabilities().quic_available
                    && req.supports_streams
                    && req.requires_encryption
            }
            TransportKind::Tcp => {
                self.capabilities().tcp_available && req.requires_reliability
            }
            TransportKind::Ice => {
                self.capabilities().ice_available && req.requires_nat_traversal
            }
        }
    }

    /// Determine preferred transport order based on capabilities
    fn determine_preferred_transports(caps: &TransportCapabilities) -> Vec<TransportKind> {
        let mut transports = Vec::new();

        // Prefer QUIC if available (provides encryption and streams)
        if caps.quic_available {
            transports.push(TransportKind::Quic);
        }

        // UDP is generally preferred for anonymity networks
        if caps.udp_available {
            transports.push(TransportKind::Udp);
        }

        // ICE for NAT traversal scenarios
        if caps.ice_available {
            transports.push(TransportKind::Ice);
        }

        // TCP as fallback
        if caps.tcp_available {
            transports.push(TransportKind::Tcp);
        }

        transports
    }
}

impl Default for TransportManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Requirements for transport selection
#[derive(Debug, Clone)]
pub struct TransportRequirements {
    pub requires_reliability: bool,
    pub allows_unreliable: bool,
    pub requires_encryption: bool,
    pub supports_streams: bool,
    pub requires_nat_traversal: bool,
    pub prefers_low_latency: bool,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn udp_available() {
        assert!(available(TransportKind::Udp));
    }

    #[test]
    fn capabilities_detection() {
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
        let a = UdpEndpoint::bind_loopback()?;
        let b = UdpEndpoint::bind_loopback()?;
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
        endpoint.setnonblocking(true)?;
        endpoint.setnonblocking(false)?;
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

// Re-export key types for convenience
pub use teredo::{perform_nat_traversal, validate_address, AddressType, TeredoAddress};

pub use stun_server::{
    // Enhanced NAT traversal types
    AdvancedNatTraversal,
    BindingRequest,
    BindingResponse,
    DetectedNatType,
    EnhancedStunServer,
    HolePunchSession,
    HolePunchState,
    IceCandidate,
    NatDetectionResult,
    NatTraversal,
    RelayStatistics,
    // Classic STUN types
    StunServer,
};
