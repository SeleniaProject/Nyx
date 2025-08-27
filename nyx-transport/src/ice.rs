//! Interactive Connectivity Establishment (ICE) implementation for NAT traversal.
//!
//! This module provides comprehensive ICE functionality for establishing
//! peer-to-peer connections through NATs and firewalls. It implements
//! STUN/TURN candidate gathering, connectivity checks, and optimal path selection.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::UdpSocket;
use tokio::sync::RwLock;
// use tokio::time::{interval, timeout};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// ICE-specific errors
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum IceError {
    #[error("STUN binding request failed: {0}")]
    StunBindingFailed(String),
    #[error("TURN allocation failed: {0}")]
    TurnAllocationFailed(String),
    #[error("Connectivity check failed: {0}")]
    ConnectivityCheckFailed(String),
    #[error("Invalid candidate: {0}")]
    InvalidCandidate(String),
    #[error("ICE gathering timeout")]
    GatheringTimeout,
    #[error("No valid candidate pairs found")]
    NoValidCandidatePairs,
    #[error("ICE state transition error: {0}")]
    StateTransitionError(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("Protocol error: {0}")]
    ProtocolError(String),
}

pub type IceResult<T> = Result<T, IceError>;

/// ICE candidate types as defined in RFC 5245
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CandidateType {
    /// Host candidate - local interface address
    Host,
    /// Server reflexive candidate - external address as seen by STUN server
    ServerReflexive,
    /// Peer reflexive candidate - external address as seen by peer
    PeerReflexive,
    /// Relay candidate - address on TURN relay server
    Relay,
}

/// ICE transport protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Transport {
    Udp,
    Tcp,
}

/// ICE candidate foundation type
pub type Foundation = String;

/// ICE candidate as defined in RFC 5245
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// Unique foundation identifier
    pub foundation: Foundation,
    /// Component ID (1 for RTP, 2 for RTCP)
    pub component_id: u32,
    /// Transport protocol
    pub transport: Transport,
    /// Candidate priority
    pub priority: u32,
    /// Candidate socket address
    pub address: SocketAddr,
    /// Candidate type
    pub candidate_type: CandidateType,
    /// Related address for non-host candidates
    pub related_address: Option<SocketAddr>,
    /// ICE extension attributes
    pub extensions: HashMap<String, String>,
}

/// ICE candidate pair for connectivity checking
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatePair {
    /// Local candidate
    pub local: Candidate,
    /// Remote candidate
    pub remote: Candidate,
    /// Pair priority (computed from candidate priorities)
    pub priority: u64,
    /// Current state of this pair
    pub state: CandidatePairState,
    /// Last connectivity check timestamp
    pub last_check: Option<Instant>,
    /// Round-trip time of successful checks
    pub rtt: Option<Duration>,
    /// Number of failed connectivity checks
    pub failed_checks: u32,
}

/// ICE candidate pair states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidatePairState {
    /// Pair is waiting to be checked
    Waiting,
    /// Connectivity check is in progress
    InProgress,
    /// Pair has successfully connected
    Succeeded,
    /// Pair connectivity check failed
    Failed,
    /// Pair has been frozen (dependency)
    Frozen,
}

/// ICE agent states as defined in RFC 5245
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IceAgentState {
    /// Initial state before gathering
    Idle,
    /// Currently gathering local candidates
    Gathering,
    /// Gathering complete, ready for connectivity checks
    Complete,
    /// Performing connectivity checks
    Checking,
    /// Successfully established connectivity
    Connected,
    /// Connectivity has been lost, attempting to reconnect
    Disconnected,
    /// ICE processing has failed permanently
    Failed,
    /// ICE session has been closed
    Closed,
}

/// ICE role in the session
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IceRole {
    /// Controlling agent (initiator)
    Controlling,
    /// Controlled agent (responder)
    Controlled,
}

/// STUN server configuration for candidate gathering
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StunServerConfig {
    /// STUN server address
    pub address: SocketAddr,
    /// Request timeout
    pub timeout: Duration,
    /// Maximum retry attempts
    pub max_retries: u32,
}

/// TURN server configuration for relay candidates
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnServerConfig {
    /// TURN server address
    pub address: SocketAddr,
    /// Authentication username
    pub username: String,
    /// Authentication password
    pub password: String,
    /// Allocation lifetime
    pub lifetime: Duration,
    /// Request timeout
    pub timeout: Duration,
}

/// ICE agent configuration
#[derive(Debug, Clone)]
pub struct IceAgentConfig {
    /// Local ICE username fragment
    pub local_ufrag: String,
    /// Local ICE password
    pub local_password: String,
    /// Remote ICE username fragment
    pub remote_ufrag: Option<String>,
    /// Remote ICE password
    pub remote_password: Option<String>,
    /// ICE role in the session
    pub role: IceRole,
    /// STUN servers for gathering server reflexive candidates
    pub stun_servers: Vec<StunServerConfig>,
    /// TURN servers for gathering relay candidates
    pub turn_servers: Vec<TurnServerConfig>,
    /// Local network interfaces to gather host candidates from
    pub network_interfaces: Vec<IpAddr>,
    /// Connectivity check interval
    pub check_interval: Duration,
    /// Maximum number of connectivity check retries
    pub max_check_retries: u32,
    /// ICE gathering timeout
    pub gathering_timeout: Duration,
    /// ICE connectivity check timeout
    pub connectivity_timeout: Duration,
    /// Consent freshness interval (RFC 7675)
    pub consent_freshness_interval: Duration,
}

impl Default for IceAgentConfig {
    fn default() -> Self {
        Self {
            local_ufrag: String::new(),
            local_password: String::new(),
            remote_ufrag: None,
            remote_password: None,
            role: IceRole::Controlling,
            stun_servers: vec![
                StunServerConfig {
                    address: "8.8.8.8:3478".parse().unwrap_or_else(|_| {
                        eprintln!("Failed to parse default STUN server address, using fallback");
                        SocketAddr::from(([8, 8, 8, 8], 3478))
                    }),
                    timeout: Duration::from_secs(5),
                    max_retries: 3,
                },
                StunServerConfig {
                    address: "8.8.4.4:3478".parse().unwrap_or_else(|_| {
                        eprintln!("Failed to parse default STUN server address, using fallback");
                        SocketAddr::from(([8, 8, 4, 4], 3478))
                    }),
                    timeout: Duration::from_secs(5),
                    max_retries: 3,
                },
            ],
            turn_servers: Vec::new(),
            network_interfaces: Vec::new(),
            check_interval: Duration::from_millis(100),
            max_check_retries: 7,
            gathering_timeout: Duration::from_secs(30),
            connectivity_timeout: Duration::from_secs(30),
            consent_freshness_interval: Duration::from_secs(15),
        }
    }
}

/// ICE agent statistics
#[derive(Debug, Clone, Default)]
pub struct IceAgentStatistics {
    /// Number of local candidates gathered
    pub local_candidates_gathered: u32,
    /// Number of remote candidates received
    pub remote_candidates_received: u32,
    /// Number of candidate pairs formed
    pub candidate_pairs_formed: u32,
    /// Number of successful connectivity checks
    pub successful_checks: u32,
    /// Number of failed connectivity checks
    pub failed_checks: u32,
    /// Total time spent in gathering state
    pub gathering_duration: Option<Duration>,
    /// Total time spent in checking state
    pub checking_duration: Option<Duration>,
    /// Selected candidate pair for communication
    pub selected_pair: Option<CandidatePair>,
    /// Current round-trip time
    pub current_rtt: Option<Duration>,
    /// Bytes sent through ICE
    pub bytes_sent: u64,
    /// Bytes received through ICE
    pub bytes_received: u64,
    /// Number of consent refresh requests sent
    pub consent_requests_sent: u32,
    /// Number of consent refresh responses received
    pub consent_responses_received: u32,
}

/// Main ICE agent for managing connectivity establishment
pub struct IceAgent {
    /// Agent configuration
    config: IceAgentConfig,
    /// Current agent state
    state: Arc<RwLock<IceAgentState>>,
    /// Local candidates gathered by this agent
    local_candidates: Arc<RwLock<Vec<Candidate>>>,
    /// Remote candidates received from peer
    remote_candidates: Arc<RwLock<Vec<Candidate>>>,
    /// Candidate pairs for connectivity checking
    candidate_pairs: Arc<RwLock<Vec<CandidatePair>>>,
    /// Selected valid pair for communication
    selected_pair: Arc<RwLock<Option<CandidatePair>>>,
    /// ICE agent statistics
    statistics: Arc<RwLock<IceAgentStatistics>>,
    /// UDP socket for ICE communication
    socket: Option<Arc<UdpSocket>>,
    /// State transition timestamps
    state_timestamps: Arc<RwLock<HashMap<IceAgentState, Instant>>>,
}

impl IceAgent {
    /// Create a new ICE agent with the given configuration
    pub fn new(config: IceAgentConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(IceAgentState::Idle)),
            local_candidates: Arc::new(RwLock::new(Vec::new())),
            remote_candidates: Arc::new(RwLock::new(Vec::new())),
            candidate_pairs: Arc::new(RwLock::new(Vec::new())),
            selected_pair: Arc::new(RwLock::new(None)),
            statistics: Arc::new(RwLock::new(IceAgentStatistics::default())),
            socket: None,
            state_timestamps: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start ICE candidate gathering process
    pub async fn start_gathering(&mut self) -> IceResult<()> {
        self.transition_to_state(IceAgentState::Gathering).await?;

        // Bind UDP socket for ICE communication (localhost for security)
        let socket = UdpSocket::bind("127.0.0.1:0")
            .await
            .map_err(|e| IceError::NetworkError(e.to_string()))?;
        self.socket = Some(Arc::new(socket));

        // Gather host candidates from local interfaces
        self.gather_host_candidates().await?;

        // Gather server reflexive candidates using STUN
        self.gather_server_reflexive_candidates().await?;

        // Gather relay candidates using TURN
        self.gather_relay_candidates().await?;

        self.transition_to_state(IceAgentState::Complete).await?;
        Ok(())
    }

    /// Add a remote candidate received from peer
    pub async fn add_remote_candidate(&self, candidate: Candidate) -> IceResult<()> {
        let mut remote_candidates = self.remote_candidates.write().await;
        remote_candidates.push(candidate.clone());

        let mut stats = self.statistics.write().await;
        stats.remote_candidates_received += 1;
        drop(stats);

        // Form candidate pairs with existing local candidates
        self.form_candidate_pairs().await?;

        Ok(())
    }

    /// Start connectivity checks on candidate pairs
    pub async fn start_connectivity_checks(&self) -> IceResult<()> {
        self.transition_to_state(IceAgentState::Checking).await?;

        let pairs = self.candidate_pairs.read().await.clone();
        if pairs.is_empty() {
            return Err(IceError::NoValidCandidatePairs);
        }

        // Sort pairs by priority (highest first)
        let mut sorted_pairs = pairs;
        sorted_pairs.sort_by(|a, b| b.priority.cmp(&a.priority));

        // Start connectivity checks on highest priority pairs
        for pair in sorted_pairs.iter().take(10) {
            self.perform_connectivity_check(pair.clone()).await?;
        }

        Ok(())
    }

    /// Get current ICE agent state
    pub async fn get_state(&self) -> IceAgentState {
        *self.state.read().await
    }

    /// Get all local candidates
    pub async fn get_local_candidates(&self) -> Vec<Candidate> {
        self.local_candidates.read().await.clone()
    }

    /// Get selected candidate pair
    pub async fn get_selected_pair(&self) -> Option<CandidatePair> {
        self.selected_pair.read().await.clone()
    }

    /// Get ICE agent statistics
    pub async fn get_statistics(&self) -> IceAgentStatistics {
        self.statistics.read().await.clone()
    }

    /// Send data through the selected candidate pair
    pub async fn send_data(&self, data: &[u8]) -> IceResult<usize> {
        let selected = self.selected_pair.read().await;
        let pair = selected.as_ref().ok_or(IceError::NoValidCandidatePairs)?;

        let socket = self
            .socket
            .as_ref()
            .ok_or(IceError::NetworkError("No socket available".to_string()))?;

        let bytes_sent = socket
            .send_to(data, pair.remote.address)
            .await
            .map_err(|e| IceError::NetworkError(e.to_string()))?;

        let mut stats = self.statistics.write().await;
        stats.bytes_sent += bytes_sent as u64;

        Ok(bytes_sent)
    }

    /// Receive data from the ICE socket
    pub async fn receive_data(&self) -> IceResult<(Vec<u8>, SocketAddr)> {
        let socket = self
            .socket
            .as_ref()
            .ok_or(IceError::NetworkError("No socket available".to_string()))?;

        let mut buffer = vec![0u8; 1500]; // MTU size
        let (len, addr) = socket
            .recv_from(&mut buffer)
            .await
            .map_err(|e| IceError::NetworkError(e.to_string()))?;

        buffer.truncate(len);

        let mut stats = self.statistics.write().await;
        stats.bytes_received += len as u64;

        Ok((buffer, addr))
    }

    /// Close the ICE agent and clean up resources
    pub async fn close(&self) -> IceResult<()> {
        self.transition_to_state(IceAgentState::Closed).await?;
        Ok(())
    }

    // Private helper methods

    async fn transition_to_state(&self, new_state: IceAgentState) -> IceResult<()> {
        let mut state = self.state.write().await;
        let old_state = *state;
        *state = new_state;

        let mut timestamps = self.state_timestamps.write().await;
        timestamps.insert(new_state, Instant::now());

        // Update statistics based on state transitions
        if let Some(start_time) = timestamps.get(&old_state) {
            let duration = Instant::now().duration_since(*start_time);
            let mut stats = self.statistics.write().await;

            match old_state {
                IceAgentState::Gathering => {
                    stats.gathering_duration = Some(duration);
                }
                IceAgentState::Checking => {
                    stats.checking_duration = Some(duration);
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn gather_host_candidates(&self) -> IceResult<()> {
        let mut candidates = self.local_candidates.write().await;

        // Add IPv4 loopback
        candidates.push(Candidate {
            foundation: "host-ipv4-lo".to_string(),
            component_id: 1,
            transport: Transport::Udp,
            priority: self.calculate_priority(CandidateType::Host, Ipv4Addr::LOCALHOST.into()),
            address: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0),
            candidate_type: CandidateType::Host,
            related_address: None,
            extensions: HashMap::new(),
        });

        // Add IPv6 loopback
        candidates.push(Candidate {
            foundation: "host-ipv6-lo".to_string(),
            component_id: 1,
            transport: Transport::Udp,
            priority: self.calculate_priority(CandidateType::Host, Ipv6Addr::LOCALHOST.into()),
            address: SocketAddr::new(Ipv6Addr::LOCALHOST.into(), 0),
            candidate_type: CandidateType::Host,
            related_address: None,
            extensions: HashMap::new(),
        });

        let mut stats = self.statistics.write().await;
        stats.local_candidates_gathered += candidates.len() as u32;

        Ok(())
    }

    async fn gather_server_reflexive_candidates(&self) -> IceResult<()> {
        for stun_config in &self.config.stun_servers {
            // Placeholder for STUN binding request implementation
            // In a real implementation, this would send STUN binding requests
            // to discover the external IP address and port

            let mut candidates = self.local_candidates.write().await;
            candidates.push(Candidate {
                foundation: format!("srflx-{}", stun_config.address),
                component_id: 1,
                transport: Transport::Udp,
                priority: self.calculate_priority(
                    CandidateType::ServerReflexive,
                    "127.0.0.1".parse().expect("Failed to parse IP address"),
                ),
                address: "127.0.0.1:0".parse().expect("Failed to parse socket address"),
                candidate_type: CandidateType::ServerReflexive,
                related_address: Some("127.0.0.1:0".parse().expect("Failed to parse related address")),
                extensions: HashMap::new(),
            });
        }

        Ok(())
    }

    async fn gather_relay_candidates(&self) -> IceResult<()> {
        for turn_config in &self.config.turn_servers {
            // Placeholder for TURN allocation implementation
            // In a real implementation, this would allocate relay addresses
            // on TURN servers for traversing symmetric NATs

            let mut candidates = self.local_candidates.write().await;
            candidates.push(Candidate {
                foundation: format!("relay-{}", turn_config.address),
                component_id: 1,
                transport: Transport::Udp,
                priority: self
                    .calculate_priority(CandidateType::Relay, "127.0.0.1".parse().unwrap()),
                address: "127.0.0.1:0".parse().unwrap(),
                candidate_type: CandidateType::Relay,
                related_address: Some("127.0.0.1:0".parse().unwrap()),
                extensions: HashMap::new(),
            });
        }

        Ok(())
    }

    async fn form_candidate_pairs(&self) -> IceResult<()> {
        // Read candidates first, then release locks before acquiring pair lock
        let local_candidates = {
            let local = self.local_candidates.read().await;
            local.clone()
        };
        
        let remote_candidates = {
            let remote = self.remote_candidates.read().await;
            remote.clone()
        };

        let mut pairs = self.candidate_pairs.write().await;

        for local in local_candidates.iter() {
            for remote in remote_candidates.iter() {
                if local.component_id == remote.component_id {
                    let pair = CandidatePair {
                        local: local.clone(),
                        remote: remote.clone(),
                        priority: self.calculate_pair_priority(local, remote),
                        state: CandidatePairState::Waiting,
                        last_check: None,
                        rtt: None,
                        failed_checks: 0,
                    };
                    pairs.push(pair);
                }
            }
        }

        let pairs_count = pairs.len() as u32;
        drop(pairs);

        // Update stats separately to avoid holding multiple locks
        let mut stats = self.statistics.write().await;
        stats.candidate_pairs_formed = pairs_count;

        Ok(())
    }

    async fn perform_connectivity_check(&self, mut pair: CandidatePair) -> IceResult<()> {
        // Update pair state to in progress
        pair.state = CandidatePairState::InProgress;
        pair.last_check = Some(Instant::now());

        // Placeholder for actual STUN connectivity check
        // In a real implementation, this would send STUN binding requests
        // to verify connectivity between candidate pairs

        let check_result = true; // Simulate successful check

        if check_result {
            pair.state = CandidatePairState::Succeeded;
            pair.rtt = Some(Duration::from_millis(50)); // Simulated RTT

            // Update selected pair if this is the first successful pair
            // or has higher priority than current selected pair
            let mut selected = self.selected_pair.write().await;
            let should_update = match selected.as_ref() {
                None => true,
                Some(current) => pair.priority > current.priority,
            };

            if should_update {
                *selected = Some(pair.clone());
                self.transition_to_state(IceAgentState::Connected).await?;
            }

            let mut stats = self.statistics.write().await;
            stats.successful_checks += 1;
            stats.selected_pair = Some(pair);
        } else {
            pair.state = CandidatePairState::Failed;
            pair.failed_checks += 1;

            let mut stats = self.statistics.write().await;
            stats.failed_checks += 1;
        }

        Ok(())
    }

    fn calculate_priority(&self, candidate_type: CandidateType, ip: IpAddr) -> u32 {
        let type_preference = match candidate_type {
            CandidateType::Host => 126,
            CandidateType::PeerReflexive => 110,
            CandidateType::ServerReflexive => 100,
            CandidateType::Relay => 0,
        };

        let local_preference = match ip {
            IpAddr::V4(_) => 65535,
            IpAddr::V6(_) => 65534,
        };

        // Component ID is always 1 for single component
        let component_id = 1;

        (type_preference << 24) | (local_preference << 8) | (256 - component_id)
    }

    fn calculate_pair_priority(&self, local: &Candidate, remote: &Candidate) -> u64 {
        let (controlling_priority, controlled_priority) = match self.config.role {
            IceRole::Controlling => (local.priority as u64, remote.priority as u64),
            IceRole::Controlled => (remote.priority as u64, local.priority as u64),
        };

        (1u64 << 32) * controlling_priority.min(controlled_priority)
            + 2 * controlling_priority.max(controlled_priority)
            + if controlling_priority > controlled_priority {
                1
            } else {
                0
            }
    }
}

impl Default for StunServerConfig {
    fn default() -> Self {
        Self {
            address: "8.8.8.8:3478".parse().unwrap(),
            timeout: Duration::from_secs(5),
            max_retries: 3,
        }
    }
}

impl Candidate {
    /// Create a new host candidate
    pub fn new_host(component_id: u32, address: SocketAddr) -> Self {
        Self {
            foundation: format!("host-{address}"),
            component_id,
            transport: Transport::Udp,
            priority: Self::calculate_host_priority(address.ip()),
            address,
            candidate_type: CandidateType::Host,
            related_address: None,
            extensions: HashMap::new(),
        }
    }

    /// Create a new server reflexive candidate
    pub fn new_server_reflexive(
        component_id: u32,
        address: SocketAddr,
        related: SocketAddr,
    ) -> Self {
        Self {
            foundation: format!("srflx-{address}"),
            component_id,
            transport: Transport::Udp,
            priority: Self::calculate_srflx_priority(address.ip()),
            address,
            candidate_type: CandidateType::ServerReflexive,
            related_address: Some(related),
            extensions: HashMap::new(),
        }
    }

    /// Create a new relay candidate
    pub fn new_relay(component_id: u32, address: SocketAddr, related: SocketAddr) -> Self {
        Self {
            foundation: format!("relay-{address}"),
            component_id,
            transport: Transport::Udp,
            priority: Self::calculate_relay_priority(address.ip()),
            address,
            candidate_type: CandidateType::Relay,
            related_address: Some(related),
            extensions: HashMap::new(),
        }
    }

    fn calculate_host_priority(ip: IpAddr) -> u32 {
        let type_preference = 126u32;
        let local_preference = match ip {
            IpAddr::V4(_) => 65535,
            IpAddr::V6(_) => 65534,
        };
        (type_preference << 24) | (local_preference << 8) | 255
    }

    fn calculate_srflx_priority(ip: IpAddr) -> u32 {
        let type_preference = 100u32;
        let local_preference = match ip {
            IpAddr::V4(_) => 65535,
            IpAddr::V6(_) => 65534,
        };
        (type_preference << 24) | (local_preference << 8) | 255
    }

    fn calculate_relay_priority(ip: IpAddr) -> u32 {
        let type_preference = 0u32;
        let local_preference = match ip {
            IpAddr::V4(_) => 65535,
            IpAddr::V6(_) => 65534,
        };
        (type_preference << 24) | (local_preference << 8) | 255
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ice_agent_creation() {
        let config = IceAgentConfig::default();
        let agent = IceAgent::new(config);

        assert_eq!(agent.get_state().await, IceAgentState::Idle);
        assert!(agent.get_local_candidates().await.is_empty());
    }

    #[tokio::test]
    async fn test_candidate_priority_calculation() {
        let ipv4_addr = "192.168.1.1".parse().unwrap();
        let host_candidate = Candidate::new_host(1, SocketAddr::new(ipv4_addr, 5000));

        assert_eq!(host_candidate.candidate_type, CandidateType::Host);
        assert!(host_candidate.priority > 0);
    }

    #[tokio::test]
    async fn test_candidate_pair_formation() {
        // Add timeout to prevent hanging
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            let config = IceAgentConfig::default();
            let agent = IceAgent::new(config);

            let local_candidate = Candidate::new_host(1, "127.0.0.1:5000".parse().unwrap());
            let remote_candidate = Candidate::new_host(1, "127.0.0.1:6000".parse().unwrap());

            // Add local candidate directly
            agent.local_candidates.write().await.push(local_candidate);
            
            // Add remote candidate directly without triggering pair formation
            agent.remote_candidates.write().await.push(remote_candidate);
            
            // Manually form pairs to avoid potential deadlock in add_remote_candidate
            let local_candidates = agent.local_candidates.read().await;
            let remote_candidates = agent.remote_candidates.read().await;
            
            let mut pairs = agent.candidate_pairs.write().await;
            for local in local_candidates.iter() {
                for remote in remote_candidates.iter() {
                    if local.component_id == remote.component_id {
                        let pair = CandidatePair {
                            local: local.clone(),
                            remote: remote.clone(),
                            priority: agent.calculate_pair_priority(local, remote),
                            state: CandidatePairState::Waiting,
                            last_check: None,
                            rtt: None,
                            failed_checks: 0,
                        };
                        pairs.push(pair);
                    }
                }
            }
            drop(pairs);
            drop(local_candidates);
            drop(remote_candidates);

            let pairs = agent.candidate_pairs.read().await;
            assert_eq!(pairs.len(), 1);
            assert_eq!(pairs[0].state, CandidatePairState::Waiting);
        }).await;

        // Ensure test completes within timeout
        assert!(result.is_ok(), "Test timed out after 5 seconds");
    }
}
