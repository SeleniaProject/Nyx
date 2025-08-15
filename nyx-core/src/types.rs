/// 256-bit node identifier (blake3(pubkey) prefix).
pub type NodeId = [u8; 32];

/// Path identifier for multipath data plane (0-255)
///
/// PathID is a critical component of the Nyx Protocol v1.0 multipath data plane.
/// It identifies which of up to 8 concurrent network paths a packet should use,
/// enabling weighted round-robin scheduling and load balancing across multiple routes.
pub type PathId = u8;

/// Connection identifier for tracking streams
pub type ConnectionId = u64;

/// Node endpoint representing a network address
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeEndpoint {
    pub ip: std::net::IpAddr,
    pub port: u16,
}

impl NodeEndpoint {
    pub fn new(addr: std::net::SocketAddr) -> Self {
        Self {
            ip: addr.ip(),
            port: addr.port(),
        }
    }

    pub fn to_socket_addr(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::new(self.ip, self.port)
    }
}

impl std::fmt::Display for NodeEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.ip, self.port)
    }
}

impl std::str::FromStr for NodeEndpoint {
    type Err = std::net::AddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let addr: std::net::SocketAddr = s.parse()?;
        Ok(Self::new(addr))
    }
}

impl From<std::net::SocketAddr> for NodeEndpoint {
    fn from(addr: std::net::SocketAddr) -> Self {
        Self::new(addr)
    }
}

impl From<NodeEndpoint> for std::net::SocketAddr {
    fn from(endpoint: NodeEndpoint) -> Self {
        endpoint.to_socket_addr()
    }
}

/// Maximum number of concurrent paths in multipath mode (v1.0 specification)
///
/// While PathID is u8 (0-255), the practical limit is 8 paths due to
/// scheduling efficiency and network resource constraints.
pub const MAX_PATHS: usize = 8;

/// Minimum number of hops in dynamic hop routing (3-7 range per v1.0 spec)
pub const MIN_HOPS: u8 = 3;

/// Maximum number of hops in dynamic hop routing (3-7 range per v1.0 spec)  
pub const MAX_HOPS: u8 = 7;

/// Default number of hops (fallback) - middle value for optimal anonymity/latency trade-off
pub const DEFAULT_HOPS: u8 = 5;

/// PathID reserved for control path (path management, capability negotiation)
pub const CONTROL_PATH_ID: PathId = 0;

/// PathID range reserved for system use (240-255)
pub const SYSTEM_PATH_ID_START: PathId = 240;
pub const SYSTEM_PATH_ID_END: PathId = 255;

/// Check if PathID is reserved for system use
pub fn is_system_path_id(path_id: PathId) -> bool {
    path_id >= SYSTEM_PATH_ID_START && path_id <= SYSTEM_PATH_ID_END
}

/// Validate PathID is within acceptable user range (1-239)
pub fn is_valid_user_path_id(path_id: PathId) -> bool {
    path_id > CONTROL_PATH_ID && path_id < SYSTEM_PATH_ID_START
}
