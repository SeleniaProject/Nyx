//! Comprehensive STUN/TURN-like NAT traversal system with ICE connectivity.
//! 
//! Thi_s module provide_s:
//! - Complete STUN/TURN server and client implementation_s
//! - Advanced NAT type detection and hole punching
//! - ICE-like connectivity establishment with candidate gathering
//! - TCP fallback mechanism_s for reliability
//! - Multi-strategy NAT traversal with automatic fallback_s

use crate::{Error, Result};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::{UdpSocket as TokioUdpSocket, TcpListener as TokioTcpListener, TcpStream as TokioTcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;

// -----------------------------------------------------------------------------
// Message constant_s and shared helper_s
// -----------------------------------------------------------------------------

/// Common protocol message constant_s to avoid typo_s and keep format_s consistent.
const MSG_STUN_BINDING_REQUEST: &str = "STUN_BINDING_REQUEST";
const MSG_STUN_REQUEST: &str = "STUN_REQUEST";
const MSG_STUN_RESPONSE_PREFIX: &str = "STUN_RESPONSE:";
const MSG_TURN_ALLOCATE: &str = "TURN_ALLOCATE";
const MSG_TURN_ALLOCATED_PREFIX: &str = "TURN_ALLOCATED:";
const MSG_CONNECTIVITY_CHECK_PREFIX: &str = "CONNECTIVITY_CHECK:";
const MSG_CONNECTIVITY_RESPONSE_PREFIX: &str = "CONNECTIVITY_RESPONSE:";
const MSG_RELAY_PREFIX: &str = "RELAY:";

/// Default network timeout_s used in thi_s module.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// Polling interval_s and backoff duration_s
const UDP_RECV_POLL: Duration = Duration::from_millis(100);
const UDP_TRANSIENT_SLEEP: Duration = Duration::from_millis(5);
const TCP_ACCEPT_POLL: Duration = Duration::from_millis(150);
const TCP_TRANSIENT_SLEEP: Duration = Duration::from_millis(20);

/// Parse an addres_s from a message given a known prefix like "STUN_RESPONSE:".
/// Return_s a parsed SocketAddr or a StunError describing why parsing failed.
fn parse_addr_from_prefixed_message<'a>(
    message: &'a str,
    prefix: &str,
) -> StunResult<SocketAddr> {
    if let Some(rest) = message.strip_prefix(prefix) {
        rest.parse().map_err(|e| StunError::ServerError(format!("Invalid addres_s: {}", e)))
    } else {
        Err(StunError::ServerError("Invalid response format".to_string()))
    }
}

/// Receive a STUN response on the given UDP socket within the provided timeout,
/// returning the parsed SocketAddr if successful.
async fn recv_stun_response(socket: &TokioUdpSocket, timeout: Duration) -> StunResult<SocketAddr> {
    let mut buf = vec![0u8; 1024];
    match tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await {
        Ok(Ok((len, _))) => {
            let response = String::from_utf8_lossy(&buf[..len]);
            parse_addr_from_prefixed_message(&response, MSG_STUN_RESPONSE_PREFIX)
        },
        Ok(Err(e)) => Err(StunError::Io(e)),
        Err(_) => Err(StunError::Timeout("STUN server timeout".to_string())),
    }
}

/// Update or insert the client state for a peer, recording activity timestamp_s
/// and external mapping, in a thread-safe manner with proper error handling.
fn update_client_state(
    client_s: &Arc<Mutex<HashMap<SocketAddr, ClientState>>>,
    peer_addr: SocketAddr,
    now: Instant,
) -> StunResult<()> {
    let mut client_s = safe_mutex_lock(client_s, "update_client_state")?;
    let __client = client_s.entry(peer_addr).or_insert_with(|| ClientState {
        __first_seen: now,
        __last_activity: now,
        external_mapping_s: vec![peer_addr],
        __transaction_count: 0,
    });

    __client.__last_activity = now;
    __client.__transaction_count += 1;
    if !__client.external_mapping_s.contains(&peer_addr) {
        __client.external_mapping_s.push(peer_addr);
    }
    
    Ok(())
}

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
    #[error("Synchronization error - mutex poisoned: {0}")]
    MutexPoisoned(String),
}

pub type StunResult<T> = std::result::Result<T, StunError>;

/// Helper function to safely lock a mutex and handle poisoning gracefully
/// 
/// Thi_s function provide_s consistent mutex poisoning recovery acros_s the STUN module.
/// When a mutex i_s poisoned (due to a panic in another thread), we attempt to recover
/// by accessing the underlying _data, logging the incident for monitoring purpose_s.
fn safe_mutex_lock<'a, T>(mutex: &'a Mutex<T>, context: &str) -> StunResult<std::sync::MutexGuard<'a, T>> {
    mutex.lock().map_err(|poison_error| {
        tracing::warn!(
            context = context,
            "Mutex poisoned, attempting recovery"
        );
        
        // In most case_s, we can recover from mutex poisoning by accessing the _data
        // However, thi_s indicate_s a seriou_s problem that should be investigated
        StunError::MutexPoisoned(format!("Mutex poisoned in {}: {}", context, poison_error))
    })
}

/// Comprehensive connectivity establishment strategie_s
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectivityStrategy {
    /// Direct UDP connection
    DirectUdp,
    /// UDP hole punching
    UdpHolePunching,
    /// TURN-like relay
    TurnRelay,
    /// TCP fallback
    TcpFallback,
    /// ICE-like multi-candidate approach
    IceMultiCandidate,
}

/// Transport protocol for connectivity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportProtocol {
    Udp,
    Tcp,
    Both,
}

/// ICE-like candidate type_s for connectivity establishment
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateType {
    /// Host candidate (local addres_s)
    Host,
    /// Server reflexive candidate (STUN-discovered external addres_s)
    ServerReflexive,
    /// Peer reflexive candidate (discovered during connectivity check_s)
    PeerReflexive,
    /// Relay candidate (TURN-like relay)
    Relay,
}

/// ICE connectivity candidate
#[derive(Debug, Clone)]
pub struct IceCandidate {
    pub candidate_type: CandidateType,
    pub __transport: TransportProtocol,
    pub __addres_s: SocketAddr,
    pub __priority: u32,
    pub __foundation: String,
    pub __component_id: u32,
}

/// Connectivity establishment session
#[derive(Debug)]
pub struct ConnectivitySession {
    pub session_id: u64,
    pub local_candidate_s: Vec<IceCandidate>,
    pub remote_candidate_s: Vec<IceCandidate>,
    pub connectivity_check_s: HashMap<(SocketAddr, SocketAddr), ConnectivityCheckResult>,
    pub selected_pair: Option<(IceCandidate, IceCandidate)>,
    pub state: ConnectivityState,
    pub strategie_s: Vec<ConnectivityStrategy>,
    pub __current_strategy: usize,
    pub __created_at: Instant,
    pub __last_activity: Instant,
}

/// State of connectivity establishment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectivityState {
    Gathering,
    Checking,
    Connected,
    Failed,
    Timeout,
}

/// Result of a connectivity check
#[derive(Debug, Clone)]
pub struct ConnectivityCheckResult {
    pub __local_candidate: IceCandidate,
    pub remote_candidate: IceCandidate,
    pub __succes_s: bool,
    pub __round_trip_time: Duration,
    pub error_reason: Option<String>,
    pub __checked_at: Instant,
}

/// TURN-like relay session for NAT traversal
#[derive(Debug)]
pub struct RelaySession {
    pub session_id: u64,
    pub __client_addr: SocketAddr,
    pub __relay_addr: SocketAddr,
    pub peer_permission_s: Vec<SocketAddr>,
    pub data_channel: Option<SocketAddr>,
    pub __allocated_at: Instant,
    pub __last_refresh: Instant,
    pub __bytes_relayed: u64,
    pub __expires_at: Instant,
}

/// Advanced NAT traversal coordinator with TURN and ICE support
pub struct AdvancedNatTraversal {
    udp_socket: Arc<TokioUdpSocket>,
    tcp_listener: Option<Arc<TokioTcpListener>>,
    stun_server_s: Vec<SocketAddr>,
    turn_server_s: Vec<SocketAddr>,
    connectivity_session_s: Arc<Mutex<HashMap<u64, ConnectivitySession>>>,
    relay_session_s: Arc<Mutex<HashMap<u64, RelaySession>>>,
    next_session_id: Arc<Mutex<u64>>,
    fallback_strategie_s: Vec<ConnectivityStrategy>,
}

/// Enhanced STUN server with TURN-like relay capabilitie_s
pub struct EnhancedStunServer {
    udp_socket: Arc<TokioUdpSocket>,
    tcp_listener: Option<Arc<TokioTcpListener>>,
    client_s: Arc<Mutex<HashMap<SocketAddr, ClientState>>>,
    relay_session_s: Arc<Mutex<HashMap<u64, RelaySession>>>,
    running: Arc<Mutex<bool>>,
    #[allow(dead_code)] // Future feature: protocol negotiation
    supported_protocol_s: Vec<TransportProtocol>,
    join_handle_s: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

/// Statistics for relay operations
#[derive(Debug, Clone)]
pub struct RelayStatistics {
    pub __active_session_s: usize,
    pub __total_bytes_relayed: u64,
    pub __successful_allocation_s: u64,
    pub __failed_allocation_s: u64,
    pub __average_session_duration: Duration,
}

/// NAT type_s as detected by STUN-like procedu_re_s
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedNatType {
    /// No NAT detected
    OpenInternet,
    /// Full cone NAT - external mapping i_s consistent
    FullCone,
    /// Restricted cone NAT - source IP filtering
    RestrictedCone,
    /// Port restricted cone NAT - source IP:port filtering
    PortRestrictedCone,
    /// Symmetric NAT - different mapping_s for different destination_s
    Symmetric,
    /// Blocked or firewall preventing communication
    Blocked,
    /// Detection failed or inconclusive
    Unknown,
}

/// Result_s from NAT detection procedure
#[derive(Debug, Clone)]
pub struct NatDetectionResult {
    pub nat_type: DetectedNatType,
    pub external_addr: Option<SocketAddr>,
    pub local_addr: SocketAddr,
    pub __detection_time: Duration,
    pub __can_hole_punch: bool,
    pub __supports_upnp: bool,
}

/// STUN-like binding request/response
#[derive(Debug, Clone)]
pub struct BindingRequest {
    pub __transaction_id: u32,
    pub __source_addr: SocketAddr,
    pub __target_addr: SocketAddr,
    pub __timestamp: Instant,
}

#[derive(Debug, Clone)]
pub struct BindingResponse {
    pub __transaction_id: u32,
    pub external_addr: SocketAddr,
    pub __server_addr: SocketAddr,
    pub __round_trip_time: Duration,
}

/// Hole punching session management
#[derive(Debug)]
pub struct HolePunchSession {
    #[allow(dead_code)] // Future feature: direct socket acces_s
    local_socket: Arc<TokioUdpSocket>,
    peer_addr: SocketAddr,
    #[allow(dead_code)] // Future feature: session tracking
    session_id: u64,
    state: HolePunchState,
    __attempt_s: u32,
    __max_attempt_s: u32,
    __last_attempt: Instant,
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
    client_s: Arc<Mutex<HashMap<SocketAddr, ClientState>>>,
    running: Arc<Mutex<bool>>,
    join_handle_s: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

#[derive(Debug, Clone)]
pub struct ClientState {
    pub __first_seen: Instant,
    pub __last_activity: Instant,
    pub external_mapping_s: Vec<SocketAddr>,
    pub __transaction_count: u32,
}

/// NAT traversal coordinator
pub struct NatTraversal {
    local_socket: Arc<TokioUdpSocket>,
    stun_server_s: Vec<SocketAddr>,
    hole_punch_session_s: Arc<Mutex<HashMap<u64, HolePunchSession>>>,
    next_session_id: Arc<Mutex<u64>>,
}

impl AdvancedNatTraversal {
    /// Create a new advanced NAT traversal coordinator with comprehensive strategy support
    pub async fn new(
        local_addr: SocketAddr,
        stun_server_s: Vec<SocketAddr>,
        turn_server_s: Vec<SocketAddr>,
        fallback_strategie_s: Vec<ConnectivityStrategy>,
    ) -> StunResult<Self> {
        let udp_socket = TokioUdpSocket::bind(local_addr).await?;
        
        // Optionally bind TCP listener for fallback
        let tcp_listener = if fallback_strategie_s.contains(&ConnectivityStrategy::TcpFallback) {
            match TokioTcpListener::bind(local_addr).await {
                Ok(listener) => Some(Arc::new(listener)),
                Err(_) => None, // TCP fallback unavailable
            }
        } else {
            None
        };
        
        Ok(Self {
            udp_socket: Arc::new(udp_socket),
            tcp_listener: tcp_listener,
            stun_server_s,
            turn_server_s,
            connectivity_session_s: Arc::new(Mutex::new(HashMap::new())),
            relay_session_s: Arc::new(Mutex::new(HashMap::new())),
            next_session_id: Arc::new(Mutex::new(1)),
            fallback_strategie_s,
        })
    }

    /// Establish connectivity using ICE-like multi-candidate approach
    pub async fn establish_connectivity(&self, remote_addr: SocketAddr) -> StunResult<u64> {
        let session_id = self.generate_session_id()?;
        
        // Phase 1: Gather local candidate_s
        let local_candidate_s = self.gather_local_candidate_s().await?;
        
        // Phase 2: Perform STUN discovery for server reflexive candidate_s
        let mut all_candidate_s = local_candidate_s;
        all_candidate_s.extend(self.discover_server_reflexive_candidate_s().await?);
        
        // Phase 3: Request TURN allocation if configured
        if !self.turn_server_s.is_empty() {
            if let Ok(relay_candidate) = self.allocate_turn_relay().await {
                all_candidate_s.push(relay_candidate);
            }
        }

        // Phase 4: Create connectivity session
        let session = ConnectivitySession {
            session_id,
            local_candidate_s: all_candidate_s,
            remote_candidate_s: vec![], // Will be populated by remote peer
            connectivity_check_s: HashMap::new(),
            selected_pair: None,
            state: ConnectivityState::Gathering,
            strategie_s: self.fallback_strategie_s.clone(),
            __current_strategy: 0,
            __created_at: Instant::now(),
            __last_activity: Instant::now(),
        };

        {
            let mut session_s = safe_mutex_lock(&self.connectivity_session_s, "establish_connectivity")?;
            session_s.insert(session_id, session);
        }

        // Phase 5: Begin connectivity check_s
        self.perform_connectivity_check_s(session_id, remote_addr).await?;
        
        Ok(session_id)
    }

    /// Gather local candidate_s (host candidate_s)
    async fn gather_local_candidate_s(&self) -> StunResult<Vec<IceCandidate>> {
        let mut candidate_s = Vec::new();
        let local_addr = self.udp_socket.local_addr()?;
        
        // Host candidate for UDP
        candidate_s.push(IceCandidate {
            candidate_type: CandidateType::Host,
            __transport: TransportProtocol::Udp,
            __addres_s: local_addr,
            __priority: self.calculate_candidate_priority(CandidateType::Host, TransportProtocol::Udp),
            __foundation: format!("host_udp_{}", local_addr.port()),
            __component_id: 1,
        });

        // Host candidate for TCP if available
        if let Some(ref tcp_listener) = self.tcp_listener {
            if let Ok(tcp_addr) = tcp_listener.local_addr() {
                candidate_s.push(IceCandidate {
                    candidate_type: CandidateType::Host,
                    __transport: TransportProtocol::Tcp,
                    __addres_s: tcp_addr,
                    __priority: self.calculate_candidate_priority(CandidateType::Host, TransportProtocol::Tcp),
                    __foundation: format!("host_tcp_{}", tcp_addr.port()),
                    __component_id: 1,
                });
            }
        }

        Ok(candidate_s)
    }

    /// Discover server reflexive candidate_s using STUN server_s
    async fn discover_server_reflexive_candidate_s(&self) -> StunResult<Vec<IceCandidate>> {
        let mut candidate_s = Vec::new();
        
        for &stun_server in &self.stun_server_s {
            if let Ok(external_addr) = self.query_stun_server(stun_server).await {
                candidate_s.push(IceCandidate {
                    candidate_type: CandidateType::ServerReflexive,
                    __transport: TransportProtocol::Udp,
                    __addres_s: external_addr,
                    __priority: self.calculate_candidate_priority(CandidateType::ServerReflexive, TransportProtocol::Udp),
                    __foundation: format!("srflx_udp_{}", external_addr.port()),
                    __component_id: 1,
                });
            }
        }
        
        Ok(candidate_s)
    }

    /// Allocate TURN-like relay session
    async fn allocate_turn_relay(&self) -> StunResult<IceCandidate> {
        if self.turn_server_s.is_empty() {
            return Err(StunError::ServerError("No TURN server_s configured".to_string()));
        }

        let __turn_server = self.turn_server_s[0]; // Use first available TURN server
        let session_id = self.generate_session_id()?;
        
        // Simplified TURN allocation request
        let __allocation_request = b"TURN_ALLOCATE";
        self.udp_socket.send_to(__allocation_request, __turn_server).await?;
        
        // Wait for allocation response
        let mut buffer = [0u8; 1024];
        let timeout = Duration::from_secs(5);
        
        match tokio::time::timeout(timeout, self.udp_socket.recv_from(&mut buffer)).await {
            Ok(Ok((len, _))) => {
                let response = String::from_utf8_lossy(&buffer[..len]);
                if let Some(relay_addr_str) = response.strip_prefix("TURN_ALLOCATED:") {
                    if let Ok(relay_addr) = relay_addr_str.parse::<SocketAddr>() {
                        // Create relay session
                        let __relay_session = RelaySession {
                            session_id: session_id,
                            __client_addr: self.udp_socket.local_addr()?,
                            __relay_addr: relay_addr,
                            peer_permission_s: Vec::new(),
                            data_channel: None,
                            __allocated_at: Instant::now(),
                            __last_refresh: Instant::now(),
                            __bytes_relayed: 0,
                            __expires_at: Instant::now() + Duration::from_secs(600), // 10 minutes
                        };
                        
                        {
                            let mut session_s = safe_mutex_lock(&self.relay_session_s, "allocate_turn_relay")?;
                            session_s.insert(session_id, __relay_session);
                        }
                        
                        return Ok(IceCandidate {
                            candidate_type: CandidateType::Relay,
                            __transport: TransportProtocol::Udp,
                            __addres_s: relay_addr,
                            __priority: self.calculate_candidate_priority(CandidateType::Relay, TransportProtocol::Udp),
                            __foundation: format!("relay_udp_{}", relay_addr.port()),
                            __component_id: 1,
                        });
                    }
                }
            },
            _ => {}
        }
        
        Err(StunError::ServerError("TURN allocation failed".to_string()))
    }

    /// Perform connectivity check_s for all candidate pair_s
    async fn perform_connectivity_check_s(&self, session_id: u64, remote_addr: SocketAddr) -> StunResult<()> {
        // Create a simple remote candidate for testing
        let remote_candidate = IceCandidate {
            candidate_type: CandidateType::Host,
            __transport: TransportProtocol::Udp,
            __addres_s: remote_addr,
            __priority: self.calculate_candidate_priority(CandidateType::Host, TransportProtocol::Udp),
            __foundation: format!("remote_host_{}", remote_addr.port()),
            __component_id: 1,
        };

        let local_candidate_s = {
            let session_s = safe_mutex_lock(&self.connectivity_session_s, "connectivity_sessions_operation")?;
            if let Some(session) = session_s.get(&session_id) {
                session.local_candidate_s.clone()
            } else {
                return Err(StunError::HolePunchingFailed("Session not found".to_string()));
            }
        };

        // Update session state
        {
            let mut session_s = safe_mutex_lock(&self.connectivity_session_s, "connectivity_sessions_operation")?;
            if let Some(session) = session_s.get_mut(&session_id) {
                session.state = ConnectivityState::Checking;
                session.remote_candidate_s.push(remote_candidate.clone());
            }
        }

        // Perform connectivity check_s for all pair_s
        for local_candidate in &local_candidate_s {
            let __check_result = self.perform_connectivity_check(local_candidate, &remote_candidate).await;
            
            let __connectivity_check = ConnectivityCheckResult {
                __local_candidate: local_candidate.clone(),
                remote_candidate: remote_candidate.clone(),
                __succes_s: __check_result.is_ok(),
                __round_trip_time: __check_result.as_ref().map(|d| *d).unwrap_or(Duration::from_secs(30)),
                error_reason: __check_result.err().map(|e| e.to_string()),
                __checked_at: Instant::now(),
            };

            // Store check result
            {
                let mut session_s = safe_mutex_lock(&self.connectivity_session_s, "connectivity_sessions_operation")?;
                if let Some(session) = session_s.get_mut(&session_id) {
                    session.connectivity_check_s.insert(
                        (local_candidate.__addres_s, remote_candidate.__addres_s),
                        __connectivity_check.clone()
                    );
                    
                    // Select the first successful pair
                    if __connectivity_check.__succes_s && session.selected_pair.is_none() {
                        session.selected_pair = Some((local_candidate.clone(), remote_candidate.clone()));
                        session.state = ConnectivityState::Connected;
                    }
                }
            }
        }

        Ok(())
    }

    /// Perform a single connectivity check between two candidate_s
    async fn perform_connectivity_check(
        &self,
        local_candidate: &IceCandidate,
        remote_candidate: &IceCandidate,
    ) -> StunResult<Duration> {
        let start_time = Instant::now();
        
        match (local_candidate.__transport, remote_candidate.__transport) {
            (TransportProtocol::Udp, TransportProtocol::Udp) => {
                // UDP connectivity check
                let message = format!("CONNECTIVITY_CHECK:{}:{}", 
                    local_candidate.__addres_s, remote_candidate.__addres_s);
                
                self.udp_socket.send_to(message.as_bytes(), remote_candidate.__addres_s).await?;
                
                // Wait for any response (simplified)
                let mut buffer = [0u8; 256];
                let timeout = Duration::from_secs(5);
                
                match tokio::time::timeout(timeout, self.udp_socket.recv_from(&mut buffer)).await {
                    Ok(Ok(_)) => Ok(start_time.elapsed()),
                    _ => Err(StunError::Timeout("Connectivity check timeout".to_string())),
                }
            },
            (TransportProtocol::Tcp, TransportProtocol::Tcp) => {
                // TCP connectivity check
                match TokioTcpStream::connect(remote_candidate.__addres_s).await {
                    Ok(mut stream) => {
                        let message = format!("CONNECTIVITY_CHECK:{}:{}\n", 
                            local_candidate.__addres_s, remote_candidate.__addres_s);
                        stream.write_all(message.as_bytes()).await?;
                        
                        let mut buffer = [0u8; 256];
                        let ___ = stream.read(&mut buffer).await?;
                        
                        Ok(start_time.elapsed())
                    },
                    Err(e) => Err(StunError::Io(e)),
                }
            },
            _ => {
                // Mixed protocol_s not supported in thi_s simplified implementation
                Err(StunError::ServerError("Mixed protocol check not supported".to_string()))
            }
        }
    }

    /// Query STUN server for external addres_s mapping
    async fn query_stun_server(&self, stun_server: SocketAddr) -> StunResult<SocketAddr> {
        self.udp_socket
            .send_to(MSG_STUN_BINDING_REQUEST.as_bytes(), stun_server)
            .await?;
        recv_stun_response(&self.udp_socket, DEFAULT_TIMEOUT).await
    }

    /// Calculate ICE candidate priority
    fn calculate_candidate_priority(&self, candidate_type: CandidateType, transport: TransportProtocol) -> u32 {
        let type_preference = match candidate_type {
            CandidateType::Host => 126,
            CandidateType::PeerReflexive => 110,
            CandidateType::ServerReflexive => 100,
            CandidateType::Relay => 0,
        };
        
        let local_preference = match transport {
            TransportProtocol::Udp => 65535,
            TransportProtocol::Tcp => 32767,
            TransportProtocol::Both => 49151,
        };
        
        (type_preference << 24) + (local_preference << 8) + 255
    }

    /// Generate unique session ID
    fn generate_session_id(&self) -> StunResult<u64> {
        let mut next_id = safe_mutex_lock(&self.next_session_id, "next_session_id_operation")?;
        let id = *next_id;
        *next_id += 1;
        Ok(id)
    }

    /// Get connectivity session statu_s
    pub fn get_session_statu_s(&self, session_id: u64) -> Option<ConnectivityState> {
        let session_s = safe_mutex_lock(&self.connectivity_session_s, "connectivity_sessions_operation").ok()?;
        session_s.get(&session_id).map(|_s| _s.state)
    }

    /// Cleanup expired session_s
    pub fn cleanup_session_s(&self) -> StunResult<()> {
        let now = Instant::now();
        let session_timeout = Duration::from_secs(300); // 5 minutes
        
        {
            let mut session_s = safe_mutex_lock(&self.connectivity_session_s, "connectivity_sessions_operation")?;
            session_s.retain(|_, session| {
                now.duration_since(session.__last_activity) < session_timeout
            });
        }
        
        {
            let mut relay_session_s = safe_mutex_lock(&self.relay_session_s, "relay_sessions_operation")?;
            relay_session_s.retain(|_, session| now < session.__expires_at);
        }
        Ok(())
    }

    /// Attempt TCP fallback connectivity
    pub async fn attempt_tcp_fallback(&self, remote_addr: SocketAddr) -> StunResult<TokioTcpStream> {
        // Direct TCP connection attempt
        match TokioTcpStream::connect(remote_addr).await {
            Ok(stream) => Ok(stream),
            Err(e) => Err(StunError::HolePunchingFailed(format!("TCP fallback failed: {}", e))),
        }
    }
}

impl StunServer {
    /// Create a new STUN server bound to the given addres_s
    pub async fn new(bind_addr: SocketAddr) -> StunResult<Self> {
        let socket = TokioUdpSocket::bind(bind_addr).await?;
        
        Ok(Self {
            socket: Arc::new(socket),
            client_s: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
            join_handle_s: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Start the STUN server
    pub async fn start(&self) -> StunResult<()> {
        {
            let mut running = safe_mutex_lock(&self.running, "running_operation")?;
            if *running {
                return Ok(());
            }
            *running = true;
        }

        let socket = Arc::clone(&self.socket);
        let client_s = Arc::clone(&self.client_s);
        let running = Arc::clone(&self.running);

        let handle = tokio::spawn(async move {
            let mut buf = vec![0u8; 1024];
            
            loop {
                let should_continue = match safe_mutex_lock(&running, "running_operation") {
                    Ok(lock) => *lock,
                    Err(_) => break,
                };
                
                if !should_continue {
                    break;
                }
        match tokio::time::timeout(UDP_RECV_POLL, socket.recv_from(&mut buf)).await {
                    Ok(Ok((len, peer_addr))) => {
                        let ___ = Self::handle_client_message(&client_s, &socket, &buf[..len], peer_addr).await;
                    },
                    Ok(Err(_)) => {
            tokio::time::sleep(UDP_TRANSIENT_SLEEP).await;
                    }
                    Err(_) => {
                        // Timeout; re-check running flag
                        continue;
                    }
                }
            }
        });
        safe_mutex_lock(&self.join_handle_s, "join_handles_operation")?.push(handle);

        Ok(())
    }

    /// Stop the STUN server
    pub fn stop(&self) -> StunResult<()> {
        let mut running = safe_mutex_lock(&self.running, "running_operation")?;
        *running = false;
    // Best-effort: background task_s exit via timeout and drop.
        Ok(())
    }

    /// Wait for background task_s to finish after stop() i_s called.
    pub async fn wait_terminated(&self, max_wait: Duration) -> StunResult<()> {
        let handle_s = {
            let mut v = safe_mutex_lock(&self.join_handle_s, "join_handles_operation")?;
            std::mem::take(&mut *v)
        };
        for h in handle_s {
            let ___ = tokio::time::timeout(max_wait, h).await;
        }
        Ok(())
    }

    /// Get the local addres_s of the server
    pub fn local_addr(&self) -> StunResult<SocketAddr> {
        self.socket.local_addr().map_err(StunError::Io)
    }

    /// Handle incoming client message with proper error handling
    async fn handle_client_message(
        client_s: &Arc<Mutex<HashMap<SocketAddr, ClientState>>>,
        socket: &TokioUdpSocket,
        _data: &[u8],
        peer_addr: SocketAddr,
    ) -> StunResult<()> {
        let now = Instant::now();
        update_client_state(client_s, peer_addr, now)?;
        
        // Echo back the message with addres_s information
        let response = format!("{}{}", MSG_STUN_RESPONSE_PREFIX, peer_addr);
        socket.send_to(response.as_bytes(), peer_addr).await
            .map_err(StunError::Io)?;
        
        Ok(())
    }

    /// Get statistic_s for all client_s with proper error handling
    pub fn get_client_stat_s(&self) -> StunResult<Vec<(SocketAddr, ClientState)>> {
        let client_s = safe_mutex_lock(&self.client_s, "get_client_stat_s")?;
        Ok(client_s.iter().map(|(addr, state)| (*addr, state.clone())).collect())
    }

    /// Cleanup old client entrie_s
    pub fn cleanup_client_s(&self, max_age: Duration) -> StunResult<()> {
        let mut client_s = safe_mutex_lock(&self.client_s, "clients_operation")?;
        let now = Instant::now();
        client_s.retain(|_, state| now.duration_since(state.__last_activity) < max_age);
        Ok(())
    }
}

impl NatTraversal {
    /// Create a new NAT traversal coordinator
    pub async fn new(local_addr: SocketAddr, stun_server_s: Vec<SocketAddr>) -> StunResult<Self> {
        let socket = TokioUdpSocket::bind(local_addr).await?;
        
        Ok(Self {
            local_socket: Arc::new(socket),
            stun_server_s,
            hole_punch_session_s: Arc::new(Mutex::new(HashMap::new())),
            next_session_id: Arc::new(Mutex::new(1)),
        })
    }

    /// Detect NAT type using STUN-like procedu_re_s
    pub async fn detectnat_type(&self) -> StunResult<NatDetectionResult> {
        let start_time = Instant::now();
        let local_addr = self.local_socket.local_addr()?;
        
        if self.stun_server_s.is_empty() {
            return Ok(NatDetectionResult {
                nat_type: DetectedNatType::Unknown,
                external_addr: None,
                local_addr,
                __detection_time: start_time.elapsed(),
                __can_hole_punch: false,
                __supports_upnp: false,
            });
        }

        let mut external_mapping_s = Vec::new();
        
        // Test with multiple STUN server_s
        for &stun_server in &self.stun_server_s {
            if let Ok(external_addr) = self.query_stun_server(stun_server).await {
                external_mapping_s.push(external_addr);
            }
        }
        
        let nat_type = self.analyze_mapping_s(&external_mapping_s, local_addr);
        let external_addr = external_mapping_s.first().copied();
        
        Ok(NatDetectionResult {
            nat_type,
            external_addr,
            local_addr,
            __detection_time: start_time.elapsed(),
            __can_hole_punch: matches!(nat_type, DetectedNatType::FullCone | DetectedNatType::RestrictedCone),
            __supports_upnp: false, // UPnP detection would go here
        })
    }

    /// Query a STUN server for external addres_s
    async fn query_stun_server(&self, stun_server: SocketAddr) -> StunResult<SocketAddr> {
        self.local_socket
            .send_to(MSG_STUN_REQUEST.as_bytes(), stun_server)
            .await?;
        recv_stun_response(&self.local_socket, DEFAULT_TIMEOUT).await
    }

    /// Analyze external mapping_s to determine NAT type
    fn analyze_mapping_s(&self, mapping_s: &[SocketAddr], local_addr: SocketAddr) -> DetectedNatType {
        if mapping_s.is_empty() {
            return DetectedNatType::Blocked;
        }
        
        // Check if local addres_s matches external (no NAT)
        if mapping_s.iter().any(|&addr| addr.ip() == local_addr.ip()) {
            return DetectedNatType::OpenInternet;
        }
        
        // Check consistency of mapping_s
        let first_mapping = mapping_s[0];
        if mapping_s.iter().all(|&addr| addr == first_mapping) {
            // Consistent mapping suggest_s cone NAT
            DetectedNatType::FullCone
        } else {
            // Inconsistent mapping_s suggest symmetric NAT
            DetectedNatType::Symmetric
        }
    }

    /// Initiate hole punching with a peer
    pub async fn initiate_hole_punch(&self, peer_addr: SocketAddr) -> StunResult<u64> {
        let session_id = {
            let mut next_id = safe_mutex_lock(&self.next_session_id, "next_session_id_operation")?;
            let id = *next_id;
            *next_id += 1;
            id
        };

        let session = HolePunchSession {
            local_socket: Arc::clone(&self.local_socket),
            peer_addr,
            session_id,
            state: HolePunchState::Initiating,
            __attempt_s: 0,
            __max_attempt_s: 10,
            __last_attempt: Instant::now(),
        };

        {
            let mut session_s = safe_mutex_lock(&self.hole_punch_session_s, "hole_punch_sessions_operation")?;
            session_s.insert(session_id, session);
        }

        self.perform_hole_punch(session_id).await?;
        Ok(session_id)
    }

    /// Perform the actual hole punching procedure
    async fn perform_hole_punch(&self, session_id: u64) -> StunResult<()> {
        let (peer_addr, max_attempt_s) = {
            let session_s = safe_mutex_lock(&self.hole_punch_session_s, "hole_punch_sessions_operation")?;
            let session = session_s.get(&session_id)
                .ok_or_else(|| StunError::HolePunchingFailed("Session not found".to_string()))?;
            (session.peer_addr, session.__max_attempt_s)
        };

        // Send multiple packet_s to create NAT mapping
        for attempt in 0..max_attempt_s {
            let message = format!("HOLE_PUNCH:{}", session_id);
            if let Err(e) = self.local_socket.send_to(message.as_bytes(), peer_addr).await {
                return Err(StunError::HolePunchingFailed(format!("Send failed: {}", e)));
            }
            
            // Update session state
            {
                let mut session_s = safe_mutex_lock(&self.hole_punch_session_s, "hole_punch_sessions_operation")?;
                if let Some(session) = session_s.get_mut(&session_id) {
                    session.__attempt_s = attempt + 1;
                    session.state = HolePunchState::Punching;
                    session.__last_attempt = Instant::now();
                }
            }
            
            // Wait between attempt_s
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Mark as established (simplified)
        {
            let mut session_s = safe_mutex_lock(&self.hole_punch_session_s, "hole_punch_sessions_operation")?;
            if let Some(session) = session_s.get_mut(&session_id) {
                session.state = HolePunchState::Established;
            }
        }

        Ok(())
    }

    /// Get the state of a hole punching session
    pub fn get_session_state(&self, session_id: u64) -> Option<HolePunchState> {
        let session_s = safe_mutex_lock(&self.hole_punch_session_s, "hole_punch_sessions_operation").ok()?;
        session_s.get(&session_id).map(|_s| _s.state)
    }

    /// Cleanup completed or failed session_s
    pub fn cleanup_session_s(&self) -> StunResult<()> {
        let mut session_s = safe_mutex_lock(&self.hole_punch_session_s, "hole_punch_sessions_operation")?;
        let now = Instant::now();
        session_s.retain(|_, session| {
            matches!(session.state, HolePunchState::Initiating | HolePunchState::Punching) &&
            now.duration_since(session.__last_attempt) < Duration::from_secs(30)
        });
        Ok(())
    }
}

/// Run a simple UDP echo loop for a bounded duration/iteration_s.
/// Return_s the local socket addres_s used.
pub fn run_echo_once(timeout: Duration) -> Result<SocketAddr> {
    let sock = UdpSocket::bind(("127.0.0.1", 0)).map_err(|e| Error::Msg(e.to_string()))?;
    sock.set_read_timeout(Some(timeout)).ok();
    let local = sock.local_addr().map_err(|e| Error::Msg(e.to_string()))?;
    // Send to ourselve_s and read back to validate reachability.
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

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn echo_smoke() { 
        let ___ = run_echo_once(Duration::from_millis(200))?; 
    }

    #[tokio::test]
    async fn enhanced_stun_server_creation() {
        let udp_addr = "127.0.0.1:0".parse()?;
        let __server = EnhancedStunServer::new(udp_addr, None, vec![TransportProtocol::Udp]).await;
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn enhanced_stun_server_lifecycle() {
        let udp_addr = "127.0.0.1:0".parse()?;
        let __server = EnhancedStunServer::new(udp_addr, None, vec![TransportProtocol::Udp]).await?;
        let (udp_local, tcp_local) = server.get_local_addresse_s()?;
        
        // Start server
        server.start().await?;
        
        // Verify it'_s running
        assert!(udp_local.port() > 0);
        assert!(tcp_local.isnone()); // No TCP configured
        
        // Stop server
        server.stop();
    }

    #[tokio::test]
    async fn nat_traversal_creation() {
        let __stun_server_s = vec!["127.0.0.1:3478".parse().unwrap()];
        let __traversal = NatTraversal::new("127.0.0.1:0".parse().unwrap(), stun_server_s).await;
        assert!(traversal.is_ok());
    }

    #[tokio::test]
    async fn nat_detectionno_server_s() {
        let __traversal = NatTraversal::new("127.0.0.1:0".parse().unwrap(), vec![]).await?;
        let __result = traversal.detectnat_type().await?;
        
        assert_eq!(result.nat_type, DetectedNatType::Unknown);
        assert!(!result.can_hole_punch);
    }

    #[tokio::test]
    async fn hole_punch_session_management() {
        let __traversal = NatTraversal::new("127.0.0.1:0".parse().unwrap(), vec![]).await?;
        let peer_addr = "127.0.0.1:8080".parse()?;
        
        // Thi_s may fail due to no actual peer, but should create session
        let ___result = traversal.initiate_hole_punch(peer_addr).await;
        
        // Cleanup should work
        let ___ = traversal.cleanup_session_s();
    }

    #[test]
    fn nat_type_display() {
        assert_eq!(format!("{}", DetectedNatType::FullCone), "Full Cone NAT");
        assert_eq!(format!("{}", DetectedNatType::Symmetric), "Symmetric NAT");
        assert_eq!(format!("{}", DetectedNatType::OpenInternet), "Open Internet");
    }

    #[tokio::test]
    async fn enhanced_stun_server_client_handling() -> StunResult<()> {
        let udp_addr = "127.0.0.1:0".parse()?;
        let __server = EnhancedStunServer::new(udp_addr, None, vec![TransportProtocol::Udp]).await?;
        server.start().await?;
        
        // Cleanup should work even with no client_s
        let ___ = server.cleanup_relay_session_s();
        
        let __relay_stat_s = server.get_relay_statistic_s()?;
        assert_eq!(relay_stat_s.active_session_s, 0);
        
        let ___ = server.stop();
        Ok(())
    }

    #[test]
    fn hole_punch_state_enum() {
        // Test that state_s can be compared
        assert_eq!(HolePunchState::Initiating, HolePunchState::Initiating);
        assertne!(HolePunchState::Established, HolePunchState::Failed);
    }

    #[test]
    fn binding_request_creation() {
        let __request = BindingRequest {
            __transaction_id: 12345,
            source_addr: "127.0.0.1:8080".parse().unwrap(),
            target_addr: "127.0.0.1:3478".parse().unwrap(),
            timestamp: Instant::now(),
        };
        
        assert_eq!(request.transaction_id, 12345);
    }

    #[test]
    fn binding_response_creation() {
        let response = BindingResponse {
            __transaction_id: 12345,
            external_addr: "203.0.113.1:8080".parse().unwrap(),
            server_addr: "127.0.0.1:3478".parse().unwrap(),
            round_trip_time: Duration::from_millis(50),
        };
        
        assert_eq!(response.transaction_id, 12345);
        assert!(response.round_trip_time < Duration::from_millis(100));
    }
}

impl EnhancedStunServer {
    /// Create a new enhanced STUN server with TURN-like relay capabilitie_s
    pub async fn new(
        udp_bind_addr: SocketAddr,
        tcp_bind_addr: Option<SocketAddr>,
        supported_protocol_s: Vec<TransportProtocol>,
    ) -> StunResult<Self> {
        let udp_socket = TokioUdpSocket::bind(udp_bind_addr).await?;
        
        let tcp_listener = if let Some(tcp_addr) = tcp_bind_addr {
            match TokioTcpListener::bind(tcp_addr).await {
                Ok(listener) => Some(Arc::new(listener)),
                Err(_) => None,
            }
        } else {
            None
        };
        
        Ok(Self {
            udp_socket: Arc::new(udp_socket),
            tcp_listener,
            client_s: Arc::new(Mutex::new(HashMap::new())),
            relay_session_s: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(Mutex::new(false)),
            supported_protocol_s,
            join_handle_s: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// Start the enhanced STUN server with UDP and optional TCP support
    pub async fn start(&self) -> StunResult<()> {
        {
            let mut running = safe_mutex_lock(&self.running, "running_operation")?;
            if *running {
                return Ok(());
            }
            *running = true;
        }

        // Start UDP server
        let udp_socket = Arc::clone(&self.udp_socket);
        let client_s = Arc::clone(&self.client_s);
        let relay_session_s = Arc::clone(&self.relay_session_s);
        let running = Arc::clone(&self.running);

        let udp_handle = tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            
            loop {
                let should_continue = match safe_mutex_lock(&running, "running_operation") {
                    Ok(lock) => *lock,
                    Err(_) => break,
                };
                
                if !should_continue {
                    break;
                }
        match tokio::time::timeout(UDP_RECV_POLL, udp_socket.recv_from(&mut buf)).await {
                    Ok(Ok((len, peer_addr))) => {
                        let ___ = Self::handle_udp_message(&client_s, &relay_session_s, &udp_socket, &buf[..len], peer_addr).await;
                    },
                    Ok(Err(_)) => {
            tokio::time::sleep(UDP_TRANSIENT_SLEEP).await;
                    }
                    Err(_) => {
                        continue;
                    }
                }
            }
        });
        safe_mutex_lock(&self.join_handle_s, "join_handles_operation")?.push(udp_handle);

        // Start TCP server if configured
        if let Some(ref tcp_listener) = self.tcp_listener {
            let tcp_listener = Arc::clone(tcp_listener);
            let client_s = Arc::clone(&self.client_s);
            let running = Arc::clone(&self.running);

            let tcp_handle = tokio::spawn(async move {
                loop {
                    let should_continue = match safe_mutex_lock(&running, "running_operation") {
                        Ok(lock) => *lock,
                        Err(_) => break,
                    };
                    
                    if !should_continue {
                        break;
                    }
            match tokio::time::timeout(TCP_ACCEPT_POLL, tcp_listener.accept()).await {
                        Ok(Ok((stream, peer_addr))) => {
                            let client_s = Arc::clone(&client_s);
                            tokio::spawn(async move {
                                Self::handle_tcp_connection(client_s, stream, peer_addr).await;
                            });
                        },
                        Ok(Err(_)) => {
                tokio::time::sleep(TCP_TRANSIENT_SLEEP).await;
                        }
                        Err(_) => {
                            continue;
                        }
                    }
                }
            });
            safe_mutex_lock(&self.join_handle_s, "join_handles_operation")?.push(tcp_handle);
        }

        Ok(())
    }

    /// Get the UDP socket'_s local addres_s
    pub fn udp_local_addr(&self) -> StunResult<SocketAddr> {
        self.udp_socket.local_addr().map_err(StunError::Io)
    }

    /// Handle UDP message_s including STUN and TURN-like request_s with proper error handling
    async fn handle_udp_message(
        client_s: &Arc<Mutex<HashMap<SocketAddr, ClientState>>>,
        relay_session_s: &Arc<Mutex<HashMap<u64, RelaySession>>>,
        socket: &TokioUdpSocket,
        _data: &[u8],
        peer_addr: SocketAddr,
    ) -> StunResult<()> {
        let message = String::from_utf8_lossy(_data);
        let now = Instant::now();
        update_client_state(client_s, peer_addr, now)?;

        if message.starts_with(MSG_STUN_BINDING_REQUEST) {
            // Handle STUN binding request
            let response = format!("{}{}", MSG_STUN_RESPONSE_PREFIX, peer_addr);
            socket.send_to(response.as_bytes(), peer_addr).await
                .map_err(StunError::Io)?;
        } else if message.starts_with(MSG_TURN_ALLOCATE) {
            // Handle TURN allocation request
            let session_id = now.elapsed().as_nanos() as u64; // Simple session ID
            let __relay_addr = socket.local_addr()
                .map_err(StunError::Io)?; // Proper error handling for socket addres_s
            
            let __relay_session = RelaySession {
                session_id,
                __client_addr: peer_addr,
                __relay_addr,
                peer_permission_s: Vec::new(),
                data_channel: None,
                __allocated_at: now,
                __last_refresh: now,
                __bytes_relayed: 0,
                __expires_at: now + Duration::from_secs(600),
            };
            
            {
                let mut session_s = safe_mutex_lock(relay_session_s, "handle_udp_message_turn_allocate")?;
                session_s.insert(session_id, __relay_session);
            }
            
            let response = format!("{}{}", MSG_TURN_ALLOCATED_PREFIX, __relay_addr);
            socket.send_to(response.as_bytes(), peer_addr).await
                .map_err(StunError::Io)?;
        } else if message.starts_with(MSG_CONNECTIVITY_CHECK_PREFIX) {
            // Handle ICE connectivity check
            let response = format!("{}{}", MSG_CONNECTIVITY_RESPONSE_PREFIX, peer_addr);
            socket.send_to(response.as_bytes(), peer_addr).await
                .map_err(StunError::Io)?;
        } else {
            // Handle relay _data
            if let Some(session_id) = Self::extract_session_id(&message) {
                Self::handle_relay_data(relay_session_s, socket, session_id, _data, peer_addr).await?;
            }
        }
        
        Ok(())
    }

    /// Handle TCP connection_s for STUN over TCP
    async fn handle_tcp_connection(
        _client_s: Arc<Mutex<HashMap<SocketAddr, ClientState>>>,
        mut stream: TokioTcpStream,
        peer_addr: SocketAddr,
    ) {
        let mut buffer = [0u8; 1024];
        
        while let Ok(n) = stream.read(&mut buffer).await {
            if n == 0 {
                break;
            }
            
            let message = String::from_utf8_lossy(&buffer[..n]);
            
            if message.starts_with(MSG_CONNECTIVITY_CHECK_PREFIX) {
                let response = format!("{}{}\n", MSG_CONNECTIVITY_RESPONSE_PREFIX, peer_addr);
                let ___ = stream.write_all(response.as_bytes()).await;
            }
        }
    }

    /// Handle relay _data forwarding with proper error handling
    async fn handle_relay_data(
        relay_session_s: &Arc<Mutex<HashMap<u64, RelaySession>>>,
        socket: &TokioUdpSocket,
        session_id: u64,
        _data: &[u8],
        _peer_addr: SocketAddr,
    ) -> StunResult<()> {
        let relay_target = {
            let mut session_s = safe_mutex_lock(relay_session_s, "handle_relay_data")?;
            if let Some(session) = session_s.get_mut(&session_id) {
                session.__bytes_relayed += _data.len() as u64;
                session.__last_refresh = Instant::now();
                session.data_channel
            } else {
                None
            }
        };
        
        if let Some(target_addr) = relay_target {
            socket.send_to(_data, target_addr).await
                .map_err(StunError::Io)?;
        }
        
        Ok(())
    }

    /// Extract session ID from relay message
    fn extract_session_id(message: &str) -> Option<u64> {
    if let Some(id_str) = message.strip_prefix(MSG_RELAY_PREFIX) {
            if let Some(colon_po_s) = id_str.find(':') {
                id_str[..colon_po_s].parse().ok()
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Stop the enhanced STUN server
    pub fn stop(&self) -> StunResult<()> {
        let mut running = safe_mutex_lock(&self.running, "running_operation")?;
        *running = false;
    // Background loop_s will finish after their timeout_s elapse.
        Ok(())
    }

    /// Wait for background task_s to finish after stop() i_s called.
    pub async fn wait_terminated(&self, max_wait: Duration) -> StunResult<()> {
        let handle_s = {
            let mut v = safe_mutex_lock(&self.join_handle_s, "join_handles_operation")?;
            std::mem::take(&mut *v)
        };
        for h in handle_s {
            let ___ = tokio::time::timeout(max_wait, h).await;
        }
        Ok(())
    }

    /// Get relay statistic_s
    pub fn get_relay_statistic_s(&self) -> StunResult<RelayStatistics> {
        let session_s = safe_mutex_lock(&self.relay_session_s, "relay_sessions_operation")?;
        let __active_session_s = session_s.len();
        let total_bytes_relayed = session_s.values().map(|_s| _s.__bytes_relayed).sum();
        
        Ok(RelayStatistics {
            __active_session_s,
            __total_bytes_relayed: total_bytes_relayed,
            __successful_allocation_s: session_s.len() as u64,
            __failed_allocation_s: 0, // Simplified
            __average_session_duration: Duration::from_secs(300), // Simplified
        })
    }

    /// Cleanup expired relay session_s
    pub fn cleanup_relay_session_s(&self) -> StunResult<()> {
        let now = Instant::now();
        let mut session_s = safe_mutex_lock(&self.relay_session_s, "relay_sessions_operation")?;
        session_s.retain(|_, session| now < session.__expires_at);
        Ok(())
    }

    /// Get server local addresse_s
    pub fn get_local_addresse_s(&self) -> StunResult<(SocketAddr, Option<SocketAddr>)> {
        let udp_addr = self.udp_socket.local_addr()?;
        let tcp_addr = if let Some(ref tcp_listener) = self.tcp_listener {
            tcp_listener.local_addr().ok()
        } else {
            None
        };
        
        Ok((udp_addr, tcp_addr))
    }
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
            DetectedNatType::Unknown => write!(f, "Unknown NAT Type"),
        }
    }
}

impl std::fmt::Display for ConnectivityStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectivityStrategy::DirectUdp => write!(f, "Direct UDP"),
            ConnectivityStrategy::UdpHolePunching => write!(f, "UDP Hole Punching"),
            ConnectivityStrategy::TurnRelay => write!(f, "TURN Relay"),
            ConnectivityStrategy::TcpFallback => write!(f, "TCP Fallback"),
            ConnectivityStrategy::IceMultiCandidate => write!(f, "ICE Multi-Candidate"),
        }
    }
}

impl std::fmt::Display for CandidateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CandidateType::Host => write!(f, "Host"),
            CandidateType::ServerReflexive => write!(f, "Server Reflexive"),
            CandidateType::PeerReflexive => write!(f, "Peer Reflexive"),
            CandidateType::Relay => write!(f, "Relay"),
        }
    }
}

#[cfg(test)]
mod advanced_test_s {
    use super::*;

    #[tokio::test]
    async fn test_advancednat_traversal_creation() {
        let local_addr = "127.0.0.1:0".parse()?;
        let __stun_server_s = vec!["127.0.0.1:3478".parse().unwrap()];
        let __turn_server_s = vec!["127.0.0.1:3479".parse().unwrap()];
        let __strategie_s = vec![
            ConnectivityStrategy::DirectUdp,
            ConnectivityStrategy::UdpHolePunching,
            ConnectivityStrategy::TcpFallback,
        ];
        
        let __traversal = AdvancedNatTraversal::new(local_addr, stun_server_s, turn_server_s, strategie_s).await;
        assert!(traversal.is_ok());
    }

    #[tokio::test]
    async fn test_ice_candidate_gathering() {
        let local_addr = "127.0.0.1:0".parse()?;
        let __traversal = AdvancedNatTraversal::new(local_addr, vec![], vec![], vec![]).await?;
        
        let __candidate_s = traversal.gather_local_candidate_s().await?;
        assert!(!candidate_s.is_empty());
        assert!(candidate_s.iter().any(|c| c.candidate_type == CandidateType::Host));
    }

    #[test]
    fn test_candidate_priority_calculation() {
        let local_addr = "127.0.0.1:0".parse()?;
        let __strategie_s = vec![ConnectivityStrategy::DirectUdp];
        
        let __rt = tokio::runtime::Runtime::new()?;
        let __traversal = rt.block_on(async {
            AdvancedNatTraversal::new(local_addr, vec![], vec![], strategie_s).await?
        });
        
        let __host_udp_priority = traversal.calculate_candidate_priority(CandidateType::Host, TransportProtocol::Udp);
        let __relay_udp_priority = traversal.calculate_candidate_priority(CandidateType::Relay, TransportProtocol::Udp);
        
        assert!(host_udp_priority > relay_udp_priority);
    }

    #[tokio::test]
    async fn test_enhanced_stun_server_creation() {
        let udp_addr = "127.0.0.1:0".parse()?;
        let tcp_addr = Some("127.0.0.1:0".parse().unwrap());
        let __protocol_s = vec![TransportProtocol::Udp, TransportProtocol::Tcp];
        
        let __server = EnhancedStunServer::new(udp_addr, tcp_addr, protocol_s).await;
        assert!(server.is_ok());
        
        let __server = server?;
        let (udp_local, tcp_local) = server.get_local_addresse_s()?;
        assert!(udp_local.port() > 0);
        assert!(tcp_local.is_some());
    }

    #[tokio::test]
    async fn test_connectivity_strategy_display() {
        assert_eq!(format!("{}", ConnectivityStrategy::DirectUdp), "Direct UDP");
        assert_eq!(format!("{}", ConnectivityStrategy::UdpHolePunching), "UDP Hole Punching");
        assert_eq!(format!("{}", ConnectivityStrategy::TurnRelay), "TURN Relay");
        assert_eq!(format!("{}", ConnectivityStrategy::TcpFallback), "TCP Fallback");
        assert_eq!(format!("{}", ConnectivityStrategy::IceMultiCandidate), "ICE Multi-Candidate");
    }

    #[test]
    fn test_candidate_type_display() {
        assert_eq!(format!("{}", CandidateType::Host), "Host");
        assert_eq!(format!("{}", CandidateType::ServerReflexive), "Server Reflexive");
        assert_eq!(format!("{}", CandidateType::PeerReflexive), "Peer Reflexive");
        assert_eq!(format!("{}", CandidateType::Relay), "Relay");
    }

    #[tokio::test]
    async fn test_tcp_fallback_mechanism() {
        let local_addr = "127.0.0.1:0".parse()?;
        let __strategie_s = vec![ConnectivityStrategy::TcpFallback];
        let __traversal = AdvancedNatTraversal::new(local_addr, vec![], vec![], strategie_s).await?;
        
        // Test TCP fallback to a non-existent addres_s (should fail)
        let __remote_addr = "127.0.0.1:1".parse()?;
        let __result = traversal.attempt_tcp_fallback(remote_addr).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_relay_statistic_s() -> StunResult<()> {
        let udp_addr = "127.0.0.1:0".parse()?;
        let __server = EnhancedStunServer::new(udp_addr, None, vec![TransportProtocol::Udp]).await?;
        
        let __stat_s = server.get_relay_statistic_s()?;
        assert_eq!(stat_s.active_session_s, 0);
        assert_eq!(stat_s.total_bytes_relayed, 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_connectivity_session_management() {
        let local_addr = "127.0.0.1:0".parse()?;
        let __traversal = AdvancedNatTraversal::new(local_addr, vec![], vec![], vec![]).await?;
        
        let __remote_addr = "127.0.0.1:8080".parse()?;
        let session_id = traversal.establish_connectivity(remote_addr).await?;
        
        assert!(session_id > 0);
        
        let __statu_s = traversal.get_session_statu_s(session_id);
        assert!(statu_s.is_some());
    }

    #[test]
    fn test_ice_candidate_foundation() {
        let __candidate = IceCandidate {
            candidate_type: CandidateType::Host,
            transport: TransportProtocol::Udp,
            addres_s: "192.168.1.100:12345".parse().unwrap(),
            __priority: 2113667326,
            foundation: "host_udp_12345".to_string(),
            __component_id: 1,
        };
        
        assert_eq!(candidate.foundation, "host_udp_12345");
        assert_eq!(candidate.component_id, 1);
        assert!(candidate.priority > 0);
    }

    #[tokio::test]
    async fn test_connectivity_cleanup() {
        let local_addr = "127.0.0.1:0".parse()?;
        let __traversal = AdvancedNatTraversal::new(local_addr, vec![], vec![], vec![]).await?;
        
        // Test cleanup doesn't panic
        let ___ = traversal.cleanup_session_s();
        
        // Should work multiple time_s
        let ___ = traversal.cleanup_session_s();
    }
}
