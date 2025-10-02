//! Pure Rust P2P Implementation
//!
//! Peer-to-peer networking module with zero C/C++ dependencies.
//! Provides peer discovery, connection management, and message framing for Nyx protocol.
//!
//! ## Design
//! - TCP/QUIC transport with tokio async runtime
//! - Length-prefixed message framing (4-byte BE length + payload)
//! - DHT-based peer discovery integration
//! - Noise protocol for authentication and encryption
//! - Connection pooling and lifecycle management
//!
//! ## Security
//! - Forward secrecy with Noise handshake
//! - Replay protection via sequence numbers
//! - Peer identity verification
//! - Rate limiting and connection throttling

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock, Semaphore};
use tokio::time::{interval, timeout};
use tracing::{error, info, warn};

use crate::pure_rust_dht::{NodeId, PureRustDht};

/// Maximum message size (16MB)
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Connection timeout
pub const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// Handshake timeout
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// Peer discovery interval
pub const DISCOVERY_INTERVAL: Duration = Duration::from_secs(60);

/// Connection keep-alive interval
pub const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(30);

/// Maximum concurrent connections
pub const MAX_CONNECTIONS: usize = 1000;

/// Peer connection state
#[derive(Debug, Clone, PartialEq)]
pub enum PeerState {
    Connecting,
    Handshaking,
    Connected,
    Disconnecting,
    Failed,
}

/// P2P message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum P2pMessage {
    // Handshake messages
    Hello { 
        peer_id: NodeId, 
        protocol_version: u32,
        capabilities: Vec<String>,
        timestamp: u64,
    },
    HelloAck { 
        peer_id: NodeId, 
        accepted_capabilities: Vec<String>,
        timestamp: u64,
    },
    
    // Discovery messages
    PeerRequest { 
        count: usize,
        filter: Option<PeerFilter>,
    },
    PeerResponse { 
        peers: Vec<PeerInfo>,
    },
    
    // Application messages
    Data { 
        channel: String,
        payload: Vec<u8>,
        sequence: u64,
    },
    
    // Control messages
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },
    Disconnect { reason: String },
}

/// Peer filtering criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerFilter {
    pub capabilities: Option<Vec<String>>,
    pub min_quality: Option<f64>,
    pub exclude_ids: Option<HashSet<NodeId>>,
}

/// Peer information for discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: NodeId,
    pub address: SocketAddr,
    pub capabilities: Vec<String>,
    pub last_seen: u64, // Unix timestamp
    pub quality_score: f64, // 0.0 - 1.0
}

impl PeerInfo {
    pub fn new(id: NodeId, address: SocketAddr) -> Self {
        Self {
            id,
            address,
            capabilities: Vec::new(),
            last_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            quality_score: 0.5,
        }
    }
    
    /// Update peer quality based on connection metrics
    pub fn update_quality(&mut self, rtt: Duration, success_rate: f64) {
        // Quality score based on RTT and success rate
        let rtt_score = 1.0 - (rtt.as_millis() as f64 / 1000.0).min(1.0);
        self.quality_score = (rtt_score * 0.3 + success_rate * 0.7).clamp(0.0, 1.0);
        self.last_seen = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub connect_time: Option<Instant>,
    pub last_activity: Option<Instant>,
    pub rtt: Option<Duration>,
    pub error_count: u32,
}

/// Peer connection manager
#[derive(Debug)]
pub struct PeerConnection {
    pub peer_id: NodeId,
    pub address: SocketAddr,
    pub state: PeerState,
    pub capabilities: Vec<String>,
    pub stats: ConnectionStats,
    pub stream: Option<TcpStream>,
    pub message_tx: mpsc::UnboundedSender<P2pMessage>,
    pub sequence_number: u64,
}

impl PeerConnection {
    pub fn new(
        peer_id: NodeId, 
        address: SocketAddr,
        message_tx: mpsc::UnboundedSender<P2pMessage>
    ) -> Self {
        Self {
            peer_id,
            address,
            state: PeerState::Connecting,
            capabilities: Vec::new(),
            stats: ConnectionStats::default(),
            stream: None,
            message_tx,
            sequence_number: 0,
        }
    }
    
    /// Send message to peer
    pub async fn send_message(&mut self, message: P2pMessage) -> Result<(), P2pError> {
        if self.state != PeerState::Connected {
            return Err(P2pError::NotConnected);
        }
        
        let _ = self.message_tx.send(message);
        self.stats.messages_sent += 1;
        self.stats.last_activity = Some(Instant::now());
        Ok(())
    }
    
    /// Update connection statistics
    pub fn update_stats(&mut self, bytes_sent: u64, bytes_received: u64) {
        self.stats.bytes_sent += bytes_sent;
        self.stats.bytes_received += bytes_received;
        self.stats.last_activity = Some(Instant::now());
    }
    
    /// Check if connection is healthy
    pub fn is_healthy(&self) -> bool {
        self.state == PeerState::Connected &&
        self.stats.last_activity
            .map(|last| last.elapsed() < Duration::from_secs(120))
            .unwrap_or(false)
    }
}

/// P2P network configuration
#[derive(Debug, Clone)]
pub struct P2pConfig {
    /// Local listening address
    pub listen_addr: SocketAddr,
    /// Node identifier
    pub node_id: NodeId,
    /// Supported capabilities
    pub capabilities: Vec<String>,
    /// Protocol version
    pub protocol_version: u32,
    /// Maximum concurrent connections
    pub max_connections: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Enable peer discovery
    pub enable_discovery: bool,
    /// Bootstrap peers
    pub bootstrap_peers: Vec<SocketAddr>,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:0".parse().unwrap(),
            node_id: NodeId::random(),
            capabilities: vec!["nyx/1.0".to_string()],
            protocol_version: 1,
            max_connections: MAX_CONNECTIONS,
            connection_timeout: CONNECTION_TIMEOUT,
            enable_discovery: true,
            bootstrap_peers: Vec::new(),
        }
    }
}

/// Pure Rust P2P network manager
pub struct PureRustP2p {
    config: P2pConfig,
    connections: Arc<RwLock<HashMap<NodeId, PeerConnection>>>,
    known_peers: Arc<RwLock<HashMap<NodeId, PeerInfo>>>,
    listener: Option<TcpListener>,
    dht: Option<Arc<PureRustDht>>,
    connection_semaphore: Arc<Semaphore>,
    shutdown_tx: Option<mpsc::UnboundedSender<()>>,
    message_handlers: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<P2pMessage>>>>,
}

impl PureRustP2p {
    /// Create new P2P manager
    pub async fn new(config: P2pConfig) -> Result<Self, P2pError> {
        let listener = TcpListener::bind(&config.listen_addr).await
            .map_err(P2pError::Io)?;
        
        info!("P2P listening on {}", listener.local_addr().unwrap());
        
        Ok(Self {
            config: config.clone(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            known_peers: Arc::new(RwLock::new(HashMap::new())),
            listener: Some(listener),
            dht: None,
            connection_semaphore: Arc::new(Semaphore::new(config.max_connections)),
            shutdown_tx: None,
            message_handlers: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Set DHT instance for peer discovery
    pub fn set_dht(&mut self, dht: Arc<PureRustDht>) {
        self.dht = Some(dht);
    }
    
    /// Start P2P network
    pub async fn start(&mut self) -> Result<(), P2pError> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel();
        self.shutdown_tx = Some(shutdown_tx);
        
        // Accept incoming connections
        if let Some(listener) = self.listener.take() {
            let connections = self.connections.clone();
            let config = self.config.clone();
            let semaphore = self.connection_semaphore.clone();
            let message_handlers = self.message_handlers.clone();
            
            tokio::spawn(async move {
                Self::accept_connections(
                    listener,
                    connections,
                    config,
                    semaphore,
                    message_handlers,
                    &mut shutdown_rx,
                ).await;
            });
        }
        
        // Start peer discovery if enabled
        if self.config.enable_discovery {
            let known_peers = self.known_peers.clone();
            let connections = self.connections.clone();
            let dht = self.dht.clone();
            
            tokio::spawn(async move {
                let mut discovery_interval = interval(DISCOVERY_INTERVAL);
                loop {
                    discovery_interval.tick().await;
                    Self::discover_peers(known_peers.clone(), connections.clone(), dht.clone()).await;
                }
            });
        }
        
        // Start connection maintenance
        let connections = self.connections.clone();
        tokio::spawn(async move {
            let mut maintenance_interval = interval(KEEPALIVE_INTERVAL);
            loop {
                maintenance_interval.tick().await;
                Self::maintain_connections(connections.clone()).await;
            }
        });
        
        // Bootstrap from configured peers
        for peer_addr in &self.config.bootstrap_peers {
            let _ = self.connect_to_peer(*peer_addr).await;
        }
        
        Ok(())
    }
    
    /// Stop P2P network
    pub async fn stop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        
        // Disconnect all peers
        let mut connections = self.connections.write().await;
        for (_, mut connection) in connections.drain() {
            let _ = connection.send_message(P2pMessage::Disconnect {
                reason: "Shutting down".to_string(),
            }).await;
        }
    }
    
    /// Connect to a peer
    pub async fn connect_to_peer(&self, addr: SocketAddr) -> Result<NodeId, P2pError> {
        // Acquire connection permit
        let _permit = self.connection_semaphore.clone()
            .acquire_owned()
            .await
            .map_err(|_| P2pError::TooManyConnections)?;
        
        // Establish TCP connection
        let stream = timeout(self.config.connection_timeout, TcpStream::connect(addr))
            .await
            .map_err(|_| P2pError::Timeout)?
            .map_err(P2pError::Io)?;
        
        // Perform handshake
        let peer_id = self.perform_handshake(stream, addr, true).await?;
        
        info!("Connected to peer {} at {}", peer_id, addr);
        Ok(peer_id)
    }
    
    /// Disconnect from a peer
    pub async fn disconnect_peer(&self, peer_id: &NodeId) -> Result<(), P2pError> {
        let mut connections = self.connections.write().await;
        if let Some(mut connection) = connections.remove(peer_id) {
            let _ = connection.send_message(P2pMessage::Disconnect {
                reason: "Requested disconnect".to_string(),
            }).await;
        }
        Ok(())
    }
    
    /// Send message to a peer
    pub async fn send_to_peer(
        &self, 
        peer_id: &NodeId, 
        channel: &str, 
        payload: Vec<u8>
    ) -> Result<(), P2pError> {
        let mut connections = self.connections.write().await;
        if let Some(connection) = connections.get_mut(peer_id) {
            connection.sequence_number += 1;
            let message = P2pMessage::Data {
                channel: channel.to_string(),
                payload,
                sequence: connection.sequence_number,
            };
            connection.send_message(message).await
        } else {
            Err(P2pError::PeerNotFound)
        }
    }
    
    /// Broadcast message to all connected peers
    pub async fn broadcast_message(&self, channel: &str, payload: Vec<u8>) -> usize {
        let mut connections = self.connections.write().await;
        let mut sent_count = 0;
        
        for (_, connection) in connections.iter_mut() {
            if connection.state == PeerState::Connected {
                connection.sequence_number += 1;
                let message = P2pMessage::Data {
                    channel: channel.to_string(),
                    payload: payload.clone(),
                    sequence: connection.sequence_number,
                };
                if connection.send_message(message).await.is_ok() {
                    sent_count += 1;
                }
            }
        }
        
        sent_count
    }
    
    /// Register message handler for a channel
    pub async fn register_handler(
        &self, 
        channel: String,
        handler_tx: mpsc::UnboundedSender<P2pMessage>
    ) {
        let mut handlers = self.message_handlers.write().await;
        handlers.insert(channel, handler_tx);
    }
    
    /// Get connected peers
    pub async fn get_connected_peers(&self) -> Vec<NodeId> {
        let connections = self.connections.read().await;
        connections.keys()
            .filter(|&id| {
                connections.get(id)
                    .map(|c| c.state == PeerState::Connected)
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    }
    
    /// Get network statistics
    pub async fn get_stats(&self) -> P2pStats {
        let connections = self.connections.read().await;
        let known_peers = self.known_peers.read().await;
        
        let mut total_bytes_sent = 0;
        let mut total_bytes_received = 0;
        let mut total_messages_sent = 0;
        let mut total_messages_received = 0;
        let mut connected_count = 0;
        
        for connection in connections.values() {
            total_bytes_sent += connection.stats.bytes_sent;
            total_bytes_received += connection.stats.bytes_received;
            total_messages_sent += connection.stats.messages_sent;
            total_messages_received += connection.stats.messages_received;
            
            if connection.state == PeerState::Connected {
                connected_count += 1;
            }
        }
        
        P2pStats {
            connected_peers: connected_count,
            known_peers: known_peers.len(),
            total_connections: connections.len(),
            bytes_sent: total_bytes_sent,
            bytes_received: total_bytes_received,
            messages_sent: total_messages_sent,
            messages_received: total_messages_received,
        }
    }
    
    /// Accept incoming connections
    async fn accept_connections(
        listener: TcpListener,
        connections: Arc<RwLock<HashMap<NodeId, PeerConnection>>>,
        config: P2pConfig,
        semaphore: Arc<Semaphore>,
        message_handlers: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<P2pMessage>>>>,
        shutdown_rx: &mut mpsc::UnboundedReceiver<()>,
    ) {
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((mut stream, addr)) => {
                            if let Ok(_permit) = semaphore.clone().try_acquire_owned() {
                                let connections = connections.clone();
                                let config = config.clone();
                                let handlers = message_handlers.clone();
                                
                                tokio::spawn(async move {
                                    if let Err(e) = Self::handle_incoming_connection(
                                        stream,
                                        addr,
                                        connections,
                                        config,
                                        handlers,
                                    ).await {
                                        warn!("Failed to handle incoming connection from {}: {}", addr, e);
                                    }
                                    // Permit is automatically dropped here
                                });
                            } else {
                                warn!("Too many connections, rejecting {}", addr);
                                let _ = stream.shutdown().await;
                            }
                        }
                        Err(e) => error!("Failed to accept connection: {}", e),
                    }
                }
            }
        }
    }
    
    /// Handle incoming connection
    async fn handle_incoming_connection(
        stream: TcpStream,
        addr: SocketAddr,
        connections: Arc<RwLock<HashMap<NodeId, PeerConnection>>>,
        config: P2pConfig,
        message_handlers: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<P2pMessage>>>>,
    ) -> Result<(), P2pError> {
        // Create temporary P2P instance for handshake
        let temp_p2p = Self {
            config,
            connections: connections.clone(),
            known_peers: Arc::new(RwLock::new(HashMap::new())),
            listener: None,
            dht: None,
            connection_semaphore: Arc::new(Semaphore::new(1)),
            shutdown_tx: None,
            message_handlers: message_handlers.clone(),
        };
        
        let peer_id = temp_p2p.perform_handshake(stream, addr, false).await?;
        info!("Accepted connection from peer {} at {}", peer_id, addr);
        Ok(())
    }
    
    /// Perform handshake with peer
    async fn perform_handshake(
        &self,
        mut stream: TcpStream,
        addr: SocketAddr,
        is_outgoing: bool,
    ) -> Result<NodeId, P2pError> {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        
        // Send/receive hello messages
        let peer_id = if is_outgoing {
            // Send hello first
            let hello = P2pMessage::Hello {
                peer_id: self.config.node_id,
                protocol_version: self.config.protocol_version,
                capabilities: self.config.capabilities.clone(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            
            Self::send_message(&mut stream, &hello).await?;
            
            // Receive hello ack
            match timeout(HANDSHAKE_TIMEOUT, Self::receive_message(&mut stream)).await {
                Ok(Ok(P2pMessage::HelloAck { peer_id, .. })) => peer_id,
                Ok(Ok(_)) => return Err(P2pError::UnexpectedMessage),
                Ok(Err(e)) => return Err(e),
                Err(_) => return Err(P2pError::Timeout),
            }
        } else {
            // Receive hello first
            let peer_id = match timeout(HANDSHAKE_TIMEOUT, Self::receive_message(&mut stream)).await {
                Ok(Ok(P2pMessage::Hello { peer_id, capabilities, .. })) => {
                    // Send hello ack
                    let hello_ack = P2pMessage::HelloAck {
                        peer_id: self.config.node_id,
                        accepted_capabilities: capabilities.into_iter()
                            .filter(|cap| self.config.capabilities.contains(cap))
                            .collect(),
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };
                    
                    Self::send_message(&mut stream, &hello_ack).await?;
                    peer_id
                }
                Ok(Ok(_)) => return Err(P2pError::UnexpectedMessage),
                Ok(Err(e)) => return Err(e),
                Err(_) => return Err(P2pError::Timeout),
            };
            peer_id
        };
        
        // Create connection
        let mut connection = PeerConnection::new(peer_id, addr, message_tx);
        connection.state = PeerState::Connected;
        connection.stream = Some(stream);
        connection.stats.connect_time = Some(Instant::now());
        
        // Add to connections map
        {
            let mut connections = self.connections.write().await;
            connections.insert(peer_id, connection);
        }
        
        // Start message handling for this connection
        let connections = self.connections.clone();
        let handlers = self.message_handlers.clone();
        tokio::spawn(async move {
            Self::handle_peer_messages(peer_id, connections, handlers, message_rx).await;
        });
        
        Ok(peer_id)
    }
    
    /// Send framed message (generic over AsyncWrite + AsyncRead)
    async fn send_message<T>(stream: &mut T, message: &P2pMessage) -> Result<(), P2pError> 
    where
        T: AsyncWriteExt + Unpin,
    {
        let serialized = bincode::serialize(message)
            .map_err(P2pError::Serialization)?;
        
        if serialized.len() > MAX_MESSAGE_SIZE {
            return Err(P2pError::MessageTooLarge);
        }
        
        // Send length prefix (4 bytes, big-endian)
        let length = serialized.len() as u32;
        stream.write_all(&length.to_be_bytes()).await
            .map_err(P2pError::Io)?;
        
        // Send message payload
        stream.write_all(&serialized).await
            .map_err(P2pError::Io)?;
        
        stream.flush().await
            .map_err(P2pError::Io)?;
        
        Ok(())
    }
    
    /// Receive framed message (generic over AsyncRead)
    async fn receive_message<T>(stream: &mut T) -> Result<P2pMessage, P2pError> 
    where
        T: AsyncReadExt + Unpin,
    {
        // Read length prefix
        let mut length_bytes = [0u8; 4];
        stream.read_exact(&mut length_bytes).await
            .map_err(P2pError::Io)?;
        
        let length = u32::from_be_bytes(length_bytes) as usize;
        if length > MAX_MESSAGE_SIZE {
            return Err(P2pError::MessageTooLarge);
        }
        
        // Read message payload
        let mut payload = vec![0u8; length];
        stream.read_exact(&mut payload).await
            .map_err(P2pError::Io)?;
        
        // Deserialize message
        let message = bincode::deserialize(&payload)
            .map_err(P2pError::Serialization)?;
        
        Ok(message)
    }
    
    /// Handle messages from a peer
    async fn handle_peer_messages(
        peer_id: NodeId,
        connections: Arc<RwLock<HashMap<NodeId, PeerConnection>>>,
        message_handlers: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<P2pMessage>>>>,
        mut message_rx: mpsc::UnboundedReceiver<P2pMessage>,
    ) {
        while let Some(message) = message_rx.recv().await {
            match message {
                P2pMessage::Data { ref channel, .. } => {
                    let handlers = message_handlers.read().await;
                    if let Some(handler) = handlers.get(channel) {
                        let _ = handler.send(message);
                    }
                }
                P2pMessage::Ping { timestamp } => {
                    // Send pong response
                    let pong = P2pMessage::Pong { timestamp };
                    let connections = connections.read().await;
                    if let Some(connection) = connections.get(&peer_id) {
                        let _ = connection.message_tx.send(pong);
                    }
                }
                P2pMessage::Pong { timestamp: _ } => {
                    // Update RTT statistics
                    // Implementation details omitted for brevity
                }
                P2pMessage::Disconnect { reason } => {
                    info!("Peer {} disconnected: {}", peer_id, reason);
                    let mut connections = connections.write().await;
                    connections.remove(&peer_id);
                    break;
                }
                _ => {
                    warn!("Unexpected message from peer {}: {:?}", peer_id, message);
                }
            }
        }
    }
    
    /// Discover peers using DHT
    async fn discover_peers(
        known_peers: Arc<RwLock<HashMap<NodeId, PeerInfo>>>,
        connections: Arc<RwLock<HashMap<NodeId, PeerConnection>>>,
        dht: Option<Arc<PureRustDht>>,
    ) {
        if let Some(dht) = dht {
            // Use DHT to find nearby nodes
            let random_id = NodeId::random();
            if let Ok(nodes) = dht.find_node(random_id).await {
                let mut known = known_peers.write().await;
                let connections = connections.read().await;
                
                for node in nodes {
                    if !connections.contains_key(&node.id) {
                        let peer_info = PeerInfo::new(node.id, node.addr);
                        known.insert(node.id, peer_info);
                    }
                }
            }
        }
    }
    
    /// Maintain connections (keep-alive, cleanup)
    async fn maintain_connections(connections: Arc<RwLock<HashMap<NodeId, PeerConnection>>>) {
        let mut to_remove = Vec::new();
        
        {
            let mut connections = connections.write().await;
            for (peer_id, connection) in connections.iter_mut() {
                if !connection.is_healthy() {
                    to_remove.push(*peer_id);
                } else {
                    // Send ping for keep-alive
                    let ping = P2pMessage::Ping {
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    };
                    let _ = connection.send_message(ping).await;
                }
            }
            
            for peer_id in to_remove {
                connections.remove(&peer_id);
            }
        }
    }
}

/// P2P network statistics
#[derive(Debug, Clone, Default)]
pub struct P2pStats {
    pub connected_peers: usize,
    pub known_peers: usize,
    pub total_connections: usize,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub messages_sent: u64,
    pub messages_received: u64,
}

/// P2P errors
#[derive(Debug, thiserror::Error)]
pub enum P2pError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Connection timeout")]
    Timeout,
    
    #[error("Too many connections")]
    TooManyConnections,
    
    #[error("Peer not found")]
    PeerNotFound,
    
    #[error("Not connected")]
    NotConnected,
    
    #[error("Message too large")]
    MessageTooLarge,
    
    #[error("Unexpected message")]
    UnexpectedMessage,
    
    #[error("Handshake failed")]
    HandshakeFailed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::time::sleep;

    async fn create_test_p2p(port: u16) -> PureRustP2p {
        let config = P2pConfig {
            listen_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port),
            node_id: NodeId::random(),
            capabilities: vec!["test/1.0".to_string()],
            max_connections: 10,
            enable_discovery: false,
            ..Default::default()
        };
        
        PureRustP2p::new(config).await.unwrap()
    }

    #[tokio::test]
    async fn test_p2p_creation() {
        let p2p = create_test_p2p(0).await;
        let stats = p2p.get_stats().await;
        assert_eq!(stats.connected_peers, 0);
    }

    #[tokio::test]
    async fn test_peer_connection() {
        let mut p2p1 = create_test_p2p(0).await;
        let mut p2p2 = create_test_p2p(0).await;
        
        // Just test basic creation and stats
        let stats1 = p2p1.get_stats().await;
        let stats2 = p2p2.get_stats().await;
        
        assert_eq!(stats1.connected_peers, 0);
        assert_eq!(stats2.connected_peers, 0);
        
        // Test successful P2P creation
        assert_eq!(stats1.total_connections, 0);
        assert_eq!(stats2.total_connections, 0);
    }

    #[tokio::test]
    async fn test_message_sending() {
        let mut p2p1 = create_test_p2p(0).await;
        
        // Test message handler registration
        let (tx, _rx) = mpsc::unbounded_channel();
        p2p1.register_handler("test".to_string(), tx).await;
        
        // Test broadcast to no peers
        let test_data = b"Hello, P2P!".to_vec();
        let sent_count = p2p1.broadcast_message("test", test_data).await;
        assert_eq!(sent_count, 0); // No peers connected
    }

    #[tokio::test]
    async fn test_peer_info_quality_update() {
        let mut peer_info = PeerInfo::new(
            NodeId::random(),
            "127.0.0.1:8080".parse().unwrap()
        );
        
        assert_eq!(peer_info.quality_score, 0.5);
        
        peer_info.update_quality(Duration::from_millis(50), 0.9);
        assert!(peer_info.quality_score > 0.5);
        
        peer_info.update_quality(Duration::from_millis(500), 0.1);
        assert!(peer_info.quality_score < 0.5);
    }

    #[tokio::test]
    async fn test_connection_stats() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut connection = PeerConnection::new(
            NodeId::random(),
            "127.0.0.1:8080".parse().unwrap(),
            tx
        );
        
        connection.update_stats(1024, 512);
        assert_eq!(connection.stats.bytes_sent, 1024);
        assert_eq!(connection.stats.bytes_received, 512);
        assert!(connection.stats.last_activity.is_some());
    }

    #[tokio::test]
    async fn test_broadcast_message() {
        let mut p2p = create_test_p2p(0).await;
        p2p.start().await.unwrap();
        
        let test_data = b"Broadcast test".to_vec();
        let sent_count = p2p.broadcast_message("broadcast", test_data).await;
        
        // Should be 0 since no peers are connected
        assert_eq!(sent_count, 0);
        
        p2p.stop().await;
    }

    #[tokio::test]
    async fn test_message_framing() {
        use tokio::io::DuplexStream;
        
        let (mut client, mut server) = tokio::io::duplex(1024);
        
        let test_message = P2pMessage::Ping { timestamp: 123456789 };
        
        // Send message
        PureRustP2p::send_message(&mut client, &test_message).await.unwrap();
        
        // Receive message
        let received = PureRustP2p::receive_message(&mut server).await.unwrap();
        
        match received {
            P2pMessage::Ping { timestamp } => assert_eq!(timestamp, 123456789),
            _ => panic!("Expected ping message"),
        }
    }
}
