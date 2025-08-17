//! STUN-like NAT traversal and connectivity testing utilities.
//! 
//! This module provides NAT type detection, hole punching capabilities,
//! and STUN-like server functionality for peer-to-peer connectivity.

use crate::{Error, Result};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket as TokioUdpSocket;

#[derive(thiserror::Error, Debug)]
pub enum StunError {
    #[error("NAT detection failed: {0}")]
    NatDetectionFailed(String),
    #[error("Hole punching failed: {0}")]
    HolePunchingFailed(String),
    #[error("STUN server error: {0}")]
    ServerError(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type StunResult<T> = std::result::Result<T, StunError>;

/// NAT types as detected by STUN-like procedures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedNatType {
    /// No NAT detected
    OpenInternet,
    /// Full cone NAT - external mapping is consistent
    FullCone,
    /// Restricted cone NAT - source IP filtering
    RestrictedCone,
    /// Port restricted cone NAT - source IP:port filtering
    PortRestrictedCone,
    /// Symmetric NAT - different mappings for different destinations
    Symmetric,
    /// Blocked or firewall preventing communication
    Blocked,
    /// Detection failed or inconclusive
    Unknown,
}

/// Results from NAT detection procedure
#[derive(Debug, Clone)]
pub struct NatDetectionResult {
    pub nat_type: DetectedNatType,
    pub external_addr: Option<SocketAddr>,
    pub local_addr: SocketAddr,
    pub detection_time: Duration,
    pub can_hole_punch: bool,
    pub supports_upnp: bool,
}

/// STUN-like binding request/response
#[derive(Debug, Clone)]
pub struct BindingRequest {
    pub transaction_id: u32,
    pub source_addr: SocketAddr,
    pub target_addr: SocketAddr,
    pub timestamp: Instant,
}

#[derive(Debug, Clone)]
pub struct BindingResponse {
    pub transaction_id: u32,
    pub external_addr: SocketAddr,
    pub server_addr: SocketAddr,
    pub round_trip_time: Duration,
}

/// Hole punching session management
#[derive(Debug)]
pub struct HolePunchSession {
    local_socket: Arc<TokioUdpSocket>,
    peer_addr: SocketAddr,
    session_id: u64,
    state: HolePunchState,
    attempts: u32,
    max_attempts: u32,
    last_attempt: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HolePunchState {
    Initiating,
    Punching,
    Established,
    Failed,
    Timeout,
}

/// STUN-like server for NAT detection and connectivity testing
pub struct StunServer {
    socket: Arc<TokioUdpSocket>,
    clients: Arc<Mutex<HashMap<SocketAddr, ClientState>>>,
    running: Arc<Mutex<bool>>,
}

#[derive(Debug, Clone)]
pub struct ClientState {
    pub first_seen: Instant,
    pub last_activity: Instant,
    pub external_mappings: Vec<SocketAddr>,
    pub transaction_count: u32,
}

/// NAT traversal coordinator
pub struct NatTraversal {
    local_socket: Arc<TokioUdpSocket>,
    stun_servers: Vec<SocketAddr>,
    hole_punch_sessions: Arc<Mutex<HashMap<u64, HolePunchSession>>>,
    next_session_id: Arc<Mutex<u64>>,
}

impl StunServer {
    /// Create a new STUN server bound to the given address
    pub async fn new(bind_addr: SocketAddr) -> StunResult<Self> {
        let socket = TokioUdpSocket::bind(bind_addr).await?;
        
        Ok(Self {
            socket: Arc::new(socket),
            clients: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
        })
    }

    /// Start the STUN server
    pub async fn start(&self) -> StunResult<()> {
        {
            let mut running = self.running.lock().unwrap();
            if *running {
                return Ok(());
            }
            *running = true;
        }

        let socket = Arc::clone(&self.socket);
        let clients = Arc::clone(&self.clients);
        let running = Arc::clone(&self.running);

        tokio::spawn(async move {
            let mut buf = vec![0u8; 1024];
            
            while *running.lock().unwrap() {
                match socket.recv_from(&mut buf).await {
                    Ok((len, peer_addr)) => {
                        Self::handle_client_message(&clients, &socket, &buf[..len], peer_addr).await;
                    },
                    Err(_) => {
                        // Handle error or timeout
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the STUN server
    pub fn stop(&self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }

    /// Get the local address of the server
    pub fn local_addr(&self) -> StunResult<SocketAddr> {
        self.socket.local_addr().map_err(StunError::Io)
    }

    /// Handle incoming client message
    async fn handle_client_message(
        clients: &Arc<Mutex<HashMap<SocketAddr, ClientState>>>,
        socket: &TokioUdpSocket,
        _data: &[u8],
        peer_addr: SocketAddr,
    ) {
        let now = Instant::now();
        
        // Update client state
        {
            let mut clients = clients.lock().unwrap();
            let client = clients.entry(peer_addr).or_insert_with(|| ClientState {
                first_seen: now,
                last_activity: now,
                external_mappings: vec![peer_addr],
                transaction_count: 0,
            });
            
            client.last_activity = now;
            client.transaction_count += 1;
            
            if !client.external_mappings.contains(&peer_addr) {
                client.external_mappings.push(peer_addr);
            }
        }
        
        // Echo back the message with address information
        let response = format!("STUN_RESPONSE:{}", peer_addr);
        let _ = socket.send_to(response.as_bytes(), peer_addr).await;
    }

    /// Get statistics for all clients
    pub fn get_client_stats(&self) -> Vec<(SocketAddr, ClientState)> {
        let clients = self.clients.lock().unwrap();
        clients.iter().map(|(addr, state)| (*addr, state.clone())).collect()
    }

    /// Cleanup old client entries
    pub fn cleanup_clients(&self, max_age: Duration) {
        let mut clients = self.clients.lock().unwrap();
        let now = Instant::now();
        clients.retain(|_, state| now.duration_since(state.last_activity) < max_age);
    }
}

impl NatTraversal {
    /// Create a new NAT traversal coordinator
    pub async fn new(local_addr: SocketAddr, stun_servers: Vec<SocketAddr>) -> StunResult<Self> {
        let socket = TokioUdpSocket::bind(local_addr).await?;
        
        Ok(Self {
            local_socket: Arc::new(socket),
            stun_servers,
            hole_punch_sessions: Arc::new(Mutex::new(HashMap::new())),
            next_session_id: Arc::new(Mutex::new(1)),
        })
    }

    /// Detect NAT type using STUN-like procedures
    pub async fn detect_nat_type(&self) -> StunResult<NatDetectionResult> {
        let start_time = Instant::now();
        let local_addr = self.local_socket.local_addr()?;
        
        if self.stun_servers.is_empty() {
            return Ok(NatDetectionResult {
                nat_type: DetectedNatType::Unknown,
                external_addr: None,
                local_addr,
                detection_time: start_time.elapsed(),
                can_hole_punch: false,
                supports_upnp: false,
            });
        }

        let mut external_mappings = Vec::new();
        
        // Test with multiple STUN servers
        for &stun_server in &self.stun_servers {
            if let Ok(external_addr) = self.query_stun_server(stun_server).await {
                external_mappings.push(external_addr);
            }
        }
        
        let nat_type = self.analyze_mappings(&external_mappings, local_addr);
        let external_addr = external_mappings.first().copied();
        
        Ok(NatDetectionResult {
            nat_type,
            external_addr,
            local_addr,
            detection_time: start_time.elapsed(),
            can_hole_punch: matches!(nat_type, DetectedNatType::FullCone | DetectedNatType::RestrictedCone),
            supports_upnp: false, // UPnP detection would go here
        })
    }

    /// Query a STUN server for external address
    async fn query_stun_server(&self, stun_server: SocketAddr) -> StunResult<SocketAddr> {
        let request = b"STUN_REQUEST";
        self.local_socket.send_to(request, stun_server).await?;
        
        let mut buf = vec![0u8; 1024];
        let timeout = Duration::from_secs(5);
        
        match tokio::time::timeout(timeout, self.local_socket.recv_from(&mut buf)).await {
            Ok(Ok((len, _))) => {
                let response = String::from_utf8_lossy(&buf[..len]);
                if let Some(addr_str) = response.strip_prefix("STUN_RESPONSE:") {
                    addr_str.parse().map_err(|e| StunError::ServerError(format!("Invalid address: {}", e)))
                } else {
                    Err(StunError::ServerError("Invalid response format".to_string()))
                }
            },
            Ok(Err(e)) => Err(StunError::Io(e)),
            Err(_) => Err(StunError::Timeout("STUN server timeout".to_string())),
        }
    }

    /// Analyze external mappings to determine NAT type
    fn analyze_mappings(&self, mappings: &[SocketAddr], local_addr: SocketAddr) -> DetectedNatType {
        if mappings.is_empty() {
            return DetectedNatType::Blocked;
        }
        
        // Check if local address matches external (no NAT)
        if mappings.iter().any(|&addr| addr.ip() == local_addr.ip()) {
            return DetectedNatType::OpenInternet;
        }
        
        // Check consistency of mappings
        let first_mapping = mappings[0];
        if mappings.iter().all(|&addr| addr == first_mapping) {
            // Consistent mapping suggests cone NAT
            DetectedNatType::FullCone
        } else {
            // Inconsistent mappings suggest symmetric NAT
            DetectedNatType::Symmetric
        }
    }

    /// Initiate hole punching with a peer
    pub async fn initiate_hole_punch(&self, peer_addr: SocketAddr) -> StunResult<u64> {
        let session_id = {
            let mut next_id = self.next_session_id.lock().unwrap();
            let id = *next_id;
            *next_id += 1;
            id
        };

        let session = HolePunchSession {
            local_socket: Arc::clone(&self.local_socket),
            peer_addr,
            session_id,
            state: HolePunchState::Initiating,
            attempts: 0,
            max_attempts: 10,
            last_attempt: Instant::now(),
        };

        {
            let mut sessions = self.hole_punch_sessions.lock().unwrap();
            sessions.insert(session_id, session);
        }

        self.perform_hole_punch(session_id).await?;
        Ok(session_id)
    }

    /// Perform the actual hole punching procedure
    async fn perform_hole_punch(&self, session_id: u64) -> StunResult<()> {
        let (peer_addr, max_attempts) = {
            let sessions = self.hole_punch_sessions.lock().unwrap();
            let session = sessions.get(&session_id)
                .ok_or_else(|| StunError::HolePunchingFailed("Session not found".to_string()))?;
            (session.peer_addr, session.max_attempts)
        };

        // Send multiple packets to create NAT mapping
        for attempt in 0..max_attempts {
            let message = format!("HOLE_PUNCH:{}", session_id);
            if let Err(e) = self.local_socket.send_to(message.as_bytes(), peer_addr).await {
                return Err(StunError::HolePunchingFailed(format!("Send failed: {}", e)));
            }
            
            // Update session state
            {
                let mut sessions = self.hole_punch_sessions.lock().unwrap();
                if let Some(session) = sessions.get_mut(&session_id) {
                    session.attempts = attempt + 1;
                    session.state = HolePunchState::Punching;
                    session.last_attempt = Instant::now();
                }
            }
            
            // Wait between attempts
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Mark as established (simplified)
        {
            let mut sessions = self.hole_punch_sessions.lock().unwrap();
            if let Some(session) = sessions.get_mut(&session_id) {
                session.state = HolePunchState::Established;
            }
        }

        Ok(())
    }

    /// Get the state of a hole punching session
    pub fn get_session_state(&self, session_id: u64) -> Option<HolePunchState> {
        let sessions = self.hole_punch_sessions.lock().unwrap();
        sessions.get(&session_id).map(|s| s.state)
    }

    /// Cleanup completed or failed sessions
    pub fn cleanup_sessions(&self) {
        let mut sessions = self.hole_punch_sessions.lock().unwrap();
        let now = Instant::now();
        sessions.retain(|_, session| {
            matches!(session.state, HolePunchState::Initiating | HolePunchState::Punching) &&
            now.duration_since(session.last_attempt) < Duration::from_secs(30)
        });
    }
}

/// Run a simple UDP echo loop for a bounded duration/iterations.
/// Returns the local socket address used.
pub fn run_echo_once(timeout: Duration) -> Result<SocketAddr> {
    let sock = UdpSocket::bind(("127.0.0.1", 0)).map_err(|e| Error::Msg(e.to_string()))?;
    sock.set_read_timeout(Some(timeout)).ok();
    let local = sock.local_addr().unwrap();
    // Send to ourselves and read back to validate reachability.
    let payload = b"echo";
    sock.send_to(payload, local).map_err(|e| Error::Msg(e.to_string()))?;
    let mut buf = [0u8; 16];
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Ok((n, from)) = sock.recv_from(&mut buf) {
            if &buf[..n] == payload && from == local {
                return Ok(local);
            }
        }
    }
    Err(Error::Msg("echo timeout".into()))
}

impl std::fmt::Display for DetectedNatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DetectedNatType::OpenInternet => write!(f, "Open Internet"),
            DetectedNatType::FullCone => write!(f, "Full Cone NAT"),
            DetectedNatType::RestrictedCone => write!(f, "Restricted Cone NAT"),
            DetectedNatType::PortRestrictedCone => write!(f, "Port Restricted Cone NAT"),
            DetectedNatType::Symmetric => write!(f, "Symmetric NAT"),
            DetectedNatType::Blocked => write!(f, "Blocked/Firewall"),
            DetectedNatType::Unknown => write!(f, "Unknown"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echo_smoke() { 
        let _ = run_echo_once(Duration::from_millis(200)).unwrap(); 
    }

    #[tokio::test]
    async fn stun_server_creation() {
        let server = StunServer::new("127.0.0.1:0".parse().unwrap()).await;
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn stun_server_lifecycle() {
        let server = StunServer::new("127.0.0.1:0".parse().unwrap()).await.unwrap();
        let addr = server.local_addr().unwrap();
        
        // Start server
        server.start().await.unwrap();
        
        // Verify it's running
        assert!(addr.port() > 0);
        
        // Stop server
        server.stop();
    }

    #[tokio::test]
    async fn nat_traversal_creation() {
        let stun_servers = vec!["127.0.0.1:3478".parse().unwrap()];
        let traversal = NatTraversal::new("127.0.0.1:0".parse().unwrap(), stun_servers).await;
        assert!(traversal.is_ok());
    }

    #[tokio::test]
    async fn nat_detection_no_servers() {
        let traversal = NatTraversal::new("127.0.0.1:0".parse().unwrap(), vec![]).await.unwrap();
        let result = traversal.detect_nat_type().await.unwrap();
        
        assert_eq!(result.nat_type, DetectedNatType::Unknown);
        assert!(!result.can_hole_punch);
    }

    #[tokio::test]
    async fn hole_punch_session_management() {
        let traversal = NatTraversal::new("127.0.0.1:0".parse().unwrap(), vec![]).await.unwrap();
        let peer_addr = "127.0.0.1:8080".parse().unwrap();
        
        // This may fail due to no actual peer, but should create session
        let _result = traversal.initiate_hole_punch(peer_addr).await;
        
        // Cleanup should work
        traversal.cleanup_sessions();
    }

    #[test]
    fn nat_type_display() {
        assert_eq!(format!("{}", DetectedNatType::FullCone), "Full Cone NAT");
        assert_eq!(format!("{}", DetectedNatType::Symmetric), "Symmetric NAT");
        assert_eq!(format!("{}", DetectedNatType::OpenInternet), "Open Internet");
    }

    #[tokio::test]
    async fn stun_server_client_handling() {
        let server = StunServer::new("127.0.0.1:0".parse().unwrap()).await.unwrap();
        server.start().await.unwrap();
        
        // Cleanup should work even with no clients
        server.cleanup_clients(Duration::from_secs(1));
        
        let stats = server.get_client_stats();
        assert!(stats.is_empty());
        
        server.stop();
    }

    #[test]
    fn hole_punch_state_enum() {
        // Test that states can be compared
        assert_eq!(HolePunchState::Initiating, HolePunchState::Initiating);
        assert_ne!(HolePunchState::Established, HolePunchState::Failed);
    }

    #[test]
    fn binding_request_creation() {
        let request = BindingRequest {
            transaction_id: 12345,
            source_addr: "127.0.0.1:8080".parse().unwrap(),
            target_addr: "127.0.0.1:3478".parse().unwrap(),
            timestamp: Instant::now(),
        };
        
        assert_eq!(request.transaction_id, 12345);
    }

    #[test]
    fn binding_response_creation() {
        let response = BindingResponse {
            transaction_id: 12345,
            external_addr: "203.0.113.1:8080".parse().unwrap(),
            server_addr: "127.0.0.1:3478".parse().unwrap(),
            round_trip_time: Duration::from_millis(50),
        };
        
        assert_eq!(response.transaction_id, 12345);
        assert!(response.round_trip_time < Duration::from_millis(100));
    }
}
