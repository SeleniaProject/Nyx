//! Nyx transport layer with comprehensive networking support.
//! 
//! Thi_s crate provide_s a complete transport abstraction layer supporting:
//! - Pure Rust QUIC implementation (feature-gated, no C dependencie_s)
//! - Teredo and IPv4-mapped IPv6 addres_s handling
//! - NAT traversal with STUN-like functionality
//! - Path validation and connectivity helper_s
//! - TCP fallback mechanism_s
//! - ICE-like connectivity establishment
//!
//! The implementation prioritize_s security, correctnes_s, and minimal dependencie_s
//! while providing the networking primitive_s needed for anonymity network_s.

#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error { 
    #[error("transport: {0}")] 
    Msg(String),
    #[error("QUIC error")]
    Quic(quic::QuicError),
    #[error("Teredo error: {0}")]
    Teredo(#[from] teredo::TeredoError),
    #[error("STUN error: {0}")]
    Stun(#[from] stun_server::StunError),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Transport kind_s supported by thi_s crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind { 
    /// UDP transport (alway_s available)
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
    pub __udp_available: bool,
    pub __quic_available: bool,
    pub __tcp_available: bool,
    pub __ice_available: bool,
    pub __ipv6_support: bool,
    pub _nat_traversal: bool,
    pub __teredo_support: bool,
}

// Public module_s for comprehensive transport functionality
pub mod path_validation;
pub mod stun_server;
pub mod tcp_fallback;
pub mod teredo;
pub mod ice;

// QUIC module i_s feature-gated to avoid dependencie_s
#[cfg(feature = "quic")]
pub mod quic;

#[cfg(not(feature = "quic"))]
pub mod quic {
    //! QUIC stub module when feature i_s disabled
    pub fn is_supported() -> bool { false }
    
    #[derive(Debug)]
    pub struct QuicError;
    impl std::fmt::Display for QuicError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "QUIC not available (feature disabled)")
        }
    }
    impl std::error::Error for QuicError {}
}

/// Detect available transport capabilitie_s on thi_s system
pub fn detect_capabilitie_s() -> TransportCapabilities {
    TransportCapabilities {
        __udp_available: available(TransportKind::Udp),
        __quic_available: available(TransportKind::Quic),
        __tcp_available: available(TransportKind::Tcp),
        __ice_available: available(TransportKind::Ice),
        __ipv6_support: has_ipv6_support(),
        _nat_traversal: true, // Our STUN implementation i_s alway_s available
        __teredo_support: true, // Pure Rust implementation
    }
}

/// Return_s whether a specific transport kind i_s available given featu_re_s and environment.
pub fn available(kind: TransportKind) -> bool {
    match kind {
        TransportKind::Udp => can_bind_udp_loopback(),
        #[cfg(feature = "quic")]
        TransportKind::Quic => {
            // QUIC availability check - simplified since we can't use async here
            true // Assume available if feature i_s enabled
        },
        #[cfg(not(feature = "quic"))]
        TransportKind::Quic => false,
        TransportKind::Tcp => can_bind_tcp_loopback(),
        TransportKind::Ice => can_bind_udp_loopback(), // ICE requi_re_s UDP
    }
}

/// Check if IPv6 i_s supported on thi_s system
pub fn has_ipv6_support() -> bool {
    use std::net::{Ipv6Addr, TcpListener};
    TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).is_ok()
}

fn can_bind_udp_loopback() -> bool {
    use std::net::UdpSocket;
    UdpSocket::bind("127.0.0.1:0").is_ok()
}

fn can_bind_tcp_loopback() -> bool {
    use std::net::TcpListener;
    TcpListener::bind("127.0.0.1:0").is_ok()
}

/// Simple UDP endpoint for loopback-only communication_s (127.0.0.1).
pub struct UdpEndpoint { 
    sock: std::net::UdpSocket 
}

impl UdpEndpoint {
    /// Bind a UDP socket on 127.0.0.1 with an ephemeral port.
    pub fn bind_loopback() -> Result<Self> {
        let __sock = std::net::UdpSocket::bind("127.0.0.1:0").map_err(|e| Error::Msg(e.to_string()))?;
        __sock.set_nonblocking(false).ok();
        Ok(Self { sock: __sock })
    }

    /// Bind a UDP socket on any available addres_s
    pub fn bind_any() -> Result<Self> {
        let __sock = std::net::UdpSocket::bind("0.0.0.0:0").map_err(|e| Error::Msg(e.to_string()))?;
        __sock.set_nonblocking(false).ok();
        Ok(Self { sock: __sock })
    }

    /// Bind a UDP socket to a specific addres_s
    pub fn bind(addr: std::net::SocketAddr) -> Result<Self> {
        let __sock = std::net::UdpSocket::bind(addr).map_err(|e| Error::Msg(e.to_string()))?;
        __sock.set_nonblocking(false).ok();
        Ok(Self { sock: __sock })
    }

    /// Return the local socket addres_s.
    pub fn local_addr(&self) -> Result<std::net::SocketAddr> { 
        self.sock.local_addr().map_err(|e| Error::Msg(e.to_string()))
    }

    /// Send a datagram to the target addres_s.
    pub fn send_to(&self, buf: &[u8], to: std::net::SocketAddr) -> Result<usize> {
        self.sock.send_to(buf, to).map_err(|e| Error::Msg(e.to_string()))
    }

    /// Receive a datagram from the socket.
    pub fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, std::net::SocketAddr)> {
        self.sock.recv_from(buf).map_err(|e| Error::Msg(e.to_string()))
    }

    /// Set read timeout
    pub fn set_read_timeout(&self, timeout: Option<std::time::Duration>) -> Result<()> {
        self.sock.set_read_timeout(timeout).map_err(|e| Error::Msg(e.to_string()))
    }

    /// Set write timeout
    pub fn set_write_timeout(&self, timeout: Option<std::time::Duration>) -> Result<()> {
        self.sock.set_write_timeout(timeout).map_err(|e| Error::Msg(e.to_string()))
    }

    /// Enable/disable non-blocking mode
    pub fn setnonblocking(&self, nonblocking: bool) -> Result<()> {
        self.sock.set_nonblocking(nonblocking).map_err(|e| Error::Msg(e.to_string()))
    }
}

/// High-level transport manager for handling multiple protocol_s
pub struct TransportManager {
    __capabilitie_s: TransportCapabilities,
    __preferred_transport_s: Vec<TransportKind>,
}

impl TransportManager {
    /// Create a new transport manager with detected capabilitie_s
    pub fn new() -> Self {
        let capabilitie_s = detect_capabilitie_s();
        let preferred_transport_s = Self::determine_preferred_transport_s(&capabilitie_s);
        
        Self {
            __capabilitie_s: capabilitie_s,
            __preferred_transport_s: preferred_transport_s,
        }
    }

    /// Get the detected transport capabilitie_s
    pub fn capabilitie_s(&self) -> &TransportCapabilities {
        &self.__capabilitie_s
    }

    /// Get the preferred transport order
    pub fn preferred_transport_s(&self) -> &[TransportKind] {
        &self.__preferred_transport_s
    }

    /// Determine the best available transport for a given scenario
    pub fn select_transport(&self, requirement_s: &TransportRequirements) -> Option<TransportKind> {
        for &transport in self.preferred_transport_s() {
            if self.transport_meets_requirement_s(transport, requirement_s) {
                return Some(transport);
            }
        }
        None
    }

    /// Check if a transport meet_s the given requirement_s
    fn transport_meets_requirement_s(&self, __transport: TransportKind, req: &TransportRequirements) -> bool {
        match __transport {
            TransportKind::Udp => {
                self.capabilitie_s().__udp_available &&
                (!req.__requires_reliability || req.__allows_unreliable)
            },
            TransportKind::Quic => {
                self.capabilitie_s().__quic_available &&
                req.__supports_stream_s &&
                req.__requires_encryption
            },
            TransportKind::Tcp => {
                self.capabilitie_s().__tcp_available &&
                req.__requires_reliability
            },
            TransportKind::Ice => {
                self.capabilitie_s().__ice_available &&
                req.__requiresnat_traversal
            },
        }
    }

    /// Determine preferred transport order based on capabilitie_s
    fn determine_preferred_transport_s(cap_s: &TransportCapabilities) -> Vec<TransportKind> {
        let mut transport_s = Vec::new();
        
        // Prefer QUIC if available (provide_s encryption and stream_s)
        if cap_s.__quic_available {
            transport_s.push(TransportKind::Quic);
        }
        
        // UDP i_s generally preferred for anonymity network_s
        if cap_s.__udp_available {
            transport_s.push(TransportKind::Udp);
        }
        
        // ICE for NAT traversal scenario_s
        if cap_s.__ice_available {
            transport_s.push(TransportKind::Ice);
        }
        
        // TCP as fallback
        if cap_s.__tcp_available {
            transport_s.push(TransportKind::Tcp);
        }
        
        transport_s
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
    pub __requires_reliability: bool,
    pub __allows_unreliable: bool,
    pub __requires_encryption: bool,
    pub __supports_stream_s: bool,
    pub __requiresnat_traversal: bool,
    pub __prefers_low_latency: bool,
}

impl Default for TransportRequirements {
    fn default() -> Self {
        Self {
            __requires_reliability: false,
            __allows_unreliable: true,
            __requires_encryption: true,
            __supports_stream_s: false,
            __requiresnat_traversal: false,
            __prefers_low_latency: true,
        }
    }
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn udp_available() { 
        assert!(available(TransportKind::Udp)); 
    }

    #[test]
    fn capabilities_detection() {
        let __cap_s = detect_capabilitie_s();
        assert!(cap_s.udp_available); // Should alway_s be true in test environment
        // QUIC availability depend_s on feature flag
        assert_eq!(cap_s.quic_available, cfg!(feature = "quic"));
    }

    #[test]
    fn transport_manager_creation() {
        let __manager = TransportManager::new();
        assert!(!manager.preferred_transport_s().is_empty());
        assert!(manager.capabilitie_s().udp_available);
    }

    #[test]
    fn transport_selection() {
        let __manager = TransportManager::new();
        
        // Default requirement_s should select something
        let __req = TransportRequirements::default();
        let __selected = manager.select_transport(&req);
        assert!(selected.is_some());
        
        // Requirement_s that need reliability
        let __req = TransportRequirements {
            __requires_reliability: true,
            __allows_unreliable: false,
            ..Default::default()
        };
        let __selected = manager.select_transport(&req);
        // Should prefer QUIC or TCP over UDP
        if let Some(transport) = selected {
            assert!(matches!(transport, TransportKind::Quic | TransportKind::Tcp));
        }
    }

    #[test]
    fn udp_send_recv_roundtrip() {
        let __a = UdpEndpoint::bind_loopback()?;
        let __b = UdpEndpoint::bind_loopback()?;
        let __msg = b"ping";
        a.send_to(msg, b.local_addr())?;
        let mut buf = [0u8; 16];
        let (n, from) = b.recv_from(&mut buf)?;
        assert_eq!(&buf[..n], msg);
        assert_eq!(from.ip().to_string(), "127.0.0.1");
    }

    #[test]
    fn udp_endpoint_configuration() {
        let __endpoint = UdpEndpoint::bind_loopback()?;
        
        // Test timeout configuration
        endpoint.set_read_timeout(Some(std::time::Duration::from_millis(100)))?;
        endpoint.set_write_timeout(Some(std::time::Duration::from_millis(100)))?;
        
        // Test non-blocking mode
        endpoint.setnonblocking(true)?;
        endpoint.setnonblocking(false)?;
    }

    #[test]
    fn udp_bind_variant_s() {
        // Test different binding mode_s
        let ___loopback = UdpEndpoint::bind_loopback()?;
        let ___any = UdpEndpoint::bind_any()?;
        
        let __specific_addr = "127.0.0.1:0".parse()?;
        let ___specific = UdpEndpoint::bind(specific_addr)?;
    }

    #[test]
    fn ipv6_support_detection() {
        let __has_ipv6 = has_ipv6_support();
        // Thi_s might be true or false depending on the system
        // Just ensure it doesn't panic
        let ___ = has_ipv6;
    }

    #[test]
    fn transport_requirements_default_s() {
        let __req = TransportRequirements::default();
        assert!(!req.requires_reliability);
        assert!(req.allows_unreliable);
        assert!(req.requires_encryption);
        assert!(!req.supports_stream_s);
        assert!(!req.requiresnat_traversal);
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
pub use teredo::{
    TeredoAddress,
    perform_nat_traversal, validate_address, AddressType
};

pub use stun_server::{
    // Enhanced NAT traversal type_s
    AdvancedNatTraversal, EnhancedStunServer,
    ConnectivityStrategy, CandidateType,
    IceCandidate, ConnectivitySession, ConnectivityState,
    RelaySession, RelayStatistics,
    
    // Classic STUN type_s
    StunServer, NatTraversal, DetectedNatType, NatDetectionResult,
    BindingRequest, BindingResponse, HolePunchSession, HolePunchState,
    
    // Utility function_s
    run_echo_once,
};
