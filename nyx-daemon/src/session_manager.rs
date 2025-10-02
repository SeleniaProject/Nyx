//! Session Manager for Nyx Daemon
//!
//! Manages active stream sessions, handshake lifecycle, and traffic key storage.
//! Integrates with nyx-stream handshake layer and provides gRPC/IPC status interface.
//!
//! ## Responsibilities
//! - Trigger hybrid post-quantum handshake for new sessions
//! - Store and manage traffic keys per session
//! - Track session state (idle, handshaking, established, closing)
//! - Provide session status to control plane (gRPC/IPC)
//! - Handle session timeouts and cleanup
//!
//! ## Architecture
//! ```text
//! ┌─────────────┐
//! │ gRPC/IPC    │
//! │ Control API │
//! └──────┬──────┘
//!        │
//!        ▼
//! ┌─────────────────┐
//! │ SessionManager  │
//! │ - create()      │
//! │ - get_status()  │
//! │ - close()       │
//! └────────┬────────┘
//!          │
//!          ▼
//! ┌─────────────────────┐
//! │ Session             │
//! │ - handshake: Client │
//! │ - traffic_keys      │
//! │ - state             │
//! │ - metrics           │
//! └─────────────────────┘
//! ```

#![forbid(unsafe_code)]

use nyx_stream::handshake::{ClientHandshake, ServerHandshake, TrafficKeys};
use nyx_stream::capability::Capability;
use nyx_stream::replay_protection::DirectionalReplayProtection;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Session identifier (32-bit)
pub type SessionId = u32;

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session created, handshake not started
    Idle,
    /// Handshake in progress (client side)
    ClientHandshaking,
    /// Handshake in progress (server side)
    ServerHandshaking,
    /// Handshake completed, traffic keys established
    Established,
    /// Session closing
    Closing,
    /// Session closed
    Closed,
    /// Session failed (handshake error, timeout, etc.)
    Failed,
}

/// Session role
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    /// This node initiated the session
    Client,
    /// This node accepted the session
    Server,
}

/// Session metrics
#[derive(Debug, Clone, Default)]
pub struct SessionMetrics {
    /// Total bytes transmitted
    pub bytes_tx: u64,
    /// Total bytes received
    pub bytes_rx: u64,
    /// Total frames transmitted
    pub frames_tx: u64,
    /// Total frames received
    pub frames_rx: u64,
    /// Handshake completion time (if completed)
    pub handshake_duration: Option<Duration>,
    /// Session establishment time
    pub established_at: Option<Instant>,
}

/// Active session
pub struct Session {
    /// Session ID
    pub id: SessionId,
    /// Session role (client or server)
    pub role: SessionRole,
    /// Current state
    pub state: SessionState,
    /// Client-side handshake (if role == Client)
    pub client_handshake: Option<ClientHandshake>,
    /// Server-side handshake (if role == Server)
    pub server_handshake: Option<ServerHandshake>,
    /// Traffic keys (set after handshake completion)
    pub traffic_keys: Option<TrafficKeys>,
    /// Anti-replay protection
    pub replay_protection: DirectionalReplayProtection,
    /// Peer capabilities (negotiated during handshake)
    pub peer_capabilities: Option<Vec<Capability>>,
    /// Session creation time
    pub created_at: Instant,
    /// Last activity time
    pub last_activity: Instant,
    /// Session metrics
    pub metrics: SessionMetrics,
}

impl Session {
    /// Create a new client-side session
    pub fn new_client(id: SessionId) -> Self {
        Self {
            id,
            role: SessionRole::Client,
            state: SessionState::Idle,
            client_handshake: Some(ClientHandshake::new()),
            server_handshake: None,
            traffic_keys: None,
            replay_protection: DirectionalReplayProtection::new(),
            peer_capabilities: None,
            created_at: Instant::now(),
            last_activity: Instant::now(),
            metrics: SessionMetrics::default(),
        }
    }

    /// Create a new server-side session
    pub fn new_server(id: SessionId) -> Self {
        Self {
            id,
            role: SessionRole::Server,
            state: SessionState::Idle,
            client_handshake: None,
            server_handshake: Some(ServerHandshake::new()),
            traffic_keys: None,
            replay_protection: DirectionalReplayProtection::new(),
            peer_capabilities: None,
            created_at: Instant::now(),
            last_activity: Instant::now(),
            metrics: SessionMetrics::default(),
        }
    }

    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Check if session is idle for timeout
    pub fn is_idle_timeout(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }

    /// Get session age
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
    
    /// Perform rekey operation
    ///
    /// This resets the anti-replay protection window as per spec:
    /// "On rekey, nonces reset to zero; the anti-replay window MUST be reset accordingly"
    pub async fn rekey(&mut self) -> Result<(), String> {
        if self.state != SessionState::Established {
            return Err(format!("Cannot rekey session in state {:?}", self.state));
        }
        
        // Reset anti-replay protection as per spec §2.1
        self.replay_protection.reset_all().await;
        
        // Update activity timestamp
        self.touch();
        
        info!("Session {} rekey completed, replay protection reset", self.id);
        Ok(())
    }
}

/// Session manager configuration
#[derive(Debug, Clone)]
pub struct SessionManagerConfig {
    /// Session idle timeout (default: 5 minutes)
    pub idle_timeout: Duration,
    /// Handshake timeout (default: 30 seconds)
    pub handshake_timeout: Duration,
    /// Maximum concurrent sessions (default: 10,000)
    pub max_sessions: usize,
    /// Enable session metrics collection
    pub enable_metrics: bool,
}

impl Default for SessionManagerConfig {
    fn default() -> Self {
        Self {
            idle_timeout: Duration::from_secs(300),      // 5 minutes
            handshake_timeout: Duration::from_secs(30),  // 30 seconds
            max_sessions: 10_000,
            enable_metrics: true,
        }
    }
}

/// Session manager
pub struct SessionManager {
    /// Active sessions
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    /// Configuration
    config: SessionManagerConfig,
    /// Next session ID
    next_session_id: Arc<RwLock<SessionId>>,
    /// Manager metrics
    metrics: Arc<RwLock<ManagerMetrics>>,
}

/// Manager-level metrics
#[derive(Debug, Clone, Default)]
pub struct ManagerMetrics {
    /// Total sessions created
    pub total_sessions_created: u64,
    /// Total handshakes succeeded
    pub handshakes_succeeded: u64,
    /// Total handshakes failed
    pub handshakes_failed: u64,
    /// Total sessions closed normally
    pub sessions_closed: u64,
    /// Total sessions timed out
    pub sessions_timed_out: u64,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(config: SessionManagerConfig) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
            next_session_id: Arc::new(RwLock::new(1)),
            metrics: Arc::new(RwLock::new(ManagerMetrics::default())),
        }
    }

    /// Create a new client-side session
    ///
    /// Returns the session ID and initiates handshake.
    pub async fn create_client_session(&self) -> Result<SessionId, SessionError> {
        let mut sessions = self.sessions.write().await;
        
        // Check max sessions limit
        if sessions.len() >= self.config.max_sessions {
            return Err(SessionError::TooManySessions);
        }

        // Allocate new session ID
        let mut next_id = self.next_session_id.write().await;
        let session_id = *next_id;
        *next_id = next_id.wrapping_add(1);
        drop(next_id);

        // Create session
        let session = Session::new_client(session_id);
        sessions.insert(session_id, session);

        // Update metrics
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.total_sessions_created += 1;
        }

        info!(session_id, "Created client session");
        Ok(session_id)
    }

    /// Create a new server-side session
    pub async fn create_server_session(&self) -> Result<SessionId, SessionError> {
        let mut sessions = self.sessions.write().await;
        
        if sessions.len() >= self.config.max_sessions {
            return Err(SessionError::TooManySessions);
        }

        let mut next_id = self.next_session_id.write().await;
        let session_id = *next_id;
        *next_id = next_id.wrapping_add(1);
        drop(next_id);

        let session = Session::new_server(session_id);
        sessions.insert(session_id, session);

        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.total_sessions_created += 1;
        }

        info!(session_id, "Created server session");
        Ok(session_id)
    }

    /// Initiate handshake for a client session
    ///
    /// Returns the hybrid public key to send in CRYPTO ClientHello frame.
    pub async fn initiate_handshake(
        &self,
        session_id: SessionId,
    ) -> Result<Vec<u8>, SessionError> {
        let mut sessions = self.sessions.write().await;
        
        let session = sessions
            .get_mut(&session_id)
            .ok_or(SessionError::SessionNotFound)?;

        // Verify role and state
        if session.role != SessionRole::Client {
            return Err(SessionError::InvalidRole);
        }
        if session.state != SessionState::Idle {
            return Err(SessionError::InvalidState);
        }

        // Get client handshake
        let client_handshake = session
            .client_handshake
            .as_mut()
            .ok_or(SessionError::HandshakeNotInitialized)?;

        // Initiate handshake
        let public_key = client_handshake
            .init()
            .await
            .map_err(|e| SessionError::HandshakeFailed(e.to_string()))?;

        // Update state
        session.state = SessionState::ClientHandshaking;
        session.touch();

        info!(session_id, "Initiated client handshake");
        Ok(public_key.to_wire_format().to_vec())
    }

    /// Process server response (ciphertext) and complete client handshake
    ///
    /// Validates server capabilities and derives traffic keys.
    pub async fn finalize_client_handshake(
        &self,
        session_id: SessionId,
        server_ciphertext: &[u8],
        server_capabilities: Option<Vec<Capability>>,
    ) -> Result<(), SessionError> {
        let mut sessions = self.sessions.write().await;
        
        let session = sessions
            .get_mut(&session_id)
            .ok_or(SessionError::SessionNotFound)?;

        if session.state != SessionState::ClientHandshaking {
            return Err(SessionError::InvalidState);
        }

        // Validate server capabilities if provided
        if let Some(caps) = &server_capabilities {
            use nyx_stream::capability::{negotiate, CapabilityError, LOCAL_CAP_IDS};
            
            if let Err(e) = negotiate(LOCAL_CAP_IDS, caps) {
                match e {
                    CapabilityError::UnsupportedRequired(cap_id) => {
                        warn!(
                            session_id,
                            unsupported_cap_id = cap_id,
                            "Server requires unsupported capability"
                        );
                        return Err(SessionError::UnsupportedCapability(cap_id));
                    }
                    _ => {
                        return Err(SessionError::CapabilityNegotiationFailed(e.to_string()));
                    }
                }
            }
            session.peer_capabilities = Some(caps.clone());
        }

        let client_handshake = session
            .client_handshake
            .as_mut()
            .ok_or(SessionError::HandshakeNotInitialized)?;

        // Parse ciphertext
        let ciphertext = nyx_crypto::HybridCiphertext::from_wire_format(server_ciphertext)
            .map_err(|e| SessionError::HandshakeFailed(e.to_string()))?;

        // Finalize handshake and derive keys
        let start = Instant::now();
        let traffic_keys = client_handshake
            .finalize(&ciphertext)
            .await
            .map_err(|e| SessionError::HandshakeFailed(e.to_string()))?;

        // Store keys and update state
        session.traffic_keys = Some(traffic_keys);
        session.state = SessionState::Established;
        session.touch();
        
        let handshake_duration = start.elapsed();
        session.metrics.handshake_duration = Some(handshake_duration);
        session.metrics.established_at = Some(Instant::now());

        // Update manager metrics
        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.handshakes_succeeded += 1;
        }

        info!(
            session_id,
            handshake_duration_ms = handshake_duration.as_millis(),
            "Client handshake completed"
        );
        Ok(())
    }

    /// Process client public key (server side)
    ///
    /// Returns the ciphertext to send in CRYPTO ServerHello frame.
    pub async fn process_client_hello(
        &self,
        session_id: SessionId,
        client_public_key: &[u8],
        client_capabilities: Option<Vec<Capability>>,
    ) -> Result<Vec<u8>, SessionError> {
        let mut sessions = self.sessions.write().await;
        
        let session = sessions
            .get_mut(&session_id)
            .ok_or(SessionError::SessionNotFound)?;

        if session.role != SessionRole::Server {
            return Err(SessionError::InvalidRole);
        }
        if session.state != SessionState::Idle {
            return Err(SessionError::InvalidState);
        }

        // Validate capabilities if provided
        if let Some(caps) = &client_capabilities {
            // Use capability module directly to preserve error details
            use nyx_stream::capability::{negotiate, CapabilityError, LOCAL_CAP_IDS};
            
            if let Err(e) = negotiate(LOCAL_CAP_IDS, caps) {
                match e {
                    CapabilityError::UnsupportedRequired(cap_id) => {
                        warn!(
                            session_id,
                            unsupported_cap_id = cap_id,
                            "Client requested unsupported required capability"
                        );
                        return Err(SessionError::UnsupportedCapability(cap_id));
                    }
                    _ => {
                        return Err(SessionError::CapabilityNegotiationFailed(e.to_string()));
                    }
                }
            }
            session.peer_capabilities = Some(caps.clone());
        }

        let server_handshake = session
            .server_handshake
            .as_mut()
            .ok_or(SessionError::HandshakeNotInitialized)?;

        // Parse public key
        let public_key = nyx_crypto::HybridPublicKey::from_wire_format(client_public_key)
            .map_err(|e| SessionError::HandshakeFailed(e.to_string()))?;

        // Process and generate response
        let ciphertext = server_handshake
            .respond(public_key)
            .await
            .map_err(|e| SessionError::HandshakeFailed(e.to_string()))?;

        session.state = SessionState::ServerHandshaking;
        session.touch();

        info!(session_id, "Processed client hello");
        Ok(ciphertext.to_wire_format())
    }

    /// Confirm server handshake completion
    pub async fn confirm_server_handshake(
        &self,
        session_id: SessionId,
    ) -> Result<(), SessionError> {
        let mut sessions = self.sessions.write().await;
        
        let session = sessions
            .get_mut(&session_id)
            .ok_or(SessionError::SessionNotFound)?;

        if session.state != SessionState::ServerHandshaking {
            return Err(SessionError::InvalidState);
        }

        let server_handshake = session
            .server_handshake
            .as_mut()
            .ok_or(SessionError::HandshakeNotInitialized)?;

        let start = Instant::now();
        let traffic_keys = server_handshake
            .confirm()
            .await
            .map_err(|e| SessionError::HandshakeFailed(e.to_string()))?;

        session.traffic_keys = Some(traffic_keys);
        session.state = SessionState::Established;
        session.touch();

        let handshake_duration = start.elapsed();
        session.metrics.handshake_duration = Some(handshake_duration);
        session.metrics.established_at = Some(Instant::now());

        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.handshakes_succeeded += 1;
        }

        info!(
            session_id,
            handshake_duration_ms = handshake_duration.as_millis(),
            "Server handshake completed"
        );
        Ok(())
    }

    /// Get session status
    pub async fn get_session_status(&self, session_id: SessionId) -> Option<SessionStatus> {
        let sessions = self.sessions.read().await;
        sessions.get(&session_id).map(|s| SessionStatus {
            id: s.id,
            role: s.role,
            state: s.state,
            age: s.age(),
            idle_time: s.last_activity.elapsed(),
            has_traffic_keys: s.traffic_keys.is_some(),
            metrics: s.metrics.clone(),
        })
    }

    /// Close a session
    pub async fn close_session(&self, session_id: SessionId) -> Result<(), SessionError> {
        let mut sessions = self.sessions.write().await;
        
        let session = sessions
            .get_mut(&session_id)
            .ok_or(SessionError::SessionNotFound)?;

        session.state = SessionState::Closing;
        session.touch();

        // Actually remove it
        sessions.remove(&session_id);

        if self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.sessions_closed += 1;
        }

        info!(session_id, "Session closed");
        Ok(())
    }

    /// Perform rekey on a session
    ///
    /// Triggers HPKE rekey and resets anti-replay protection.
    pub async fn rekey_session(&self, session_id: SessionId) -> Result<(), SessionError> {
        let mut sessions = self.sessions.write().await;
        
        let session = sessions
            .get_mut(&session_id)
            .ok_or(SessionError::SessionNotFound)?;
        
        session
            .rekey()
            .await
            .map_err(|e| SessionError::HandshakeFailed(e))?;
        
        info!(session_id, "Session rekey triggered");
        Ok(())
    }

    /// Cleanup idle sessions
    ///
    /// Should be called periodically (e.g., every minute).
    pub async fn cleanup_idle_sessions(&self) -> usize {
        let mut sessions = self.sessions.write().await;
        let mut removed = 0;

        sessions.retain(|id, session| {
            let should_remove = match session.state {
                SessionState::Idle => session.is_idle_timeout(self.config.handshake_timeout),
                SessionState::ClientHandshaking | SessionState::ServerHandshaking => {
                    session.is_idle_timeout(self.config.handshake_timeout)
                }
                SessionState::Established => session.is_idle_timeout(self.config.idle_timeout),
                SessionState::Closing | SessionState::Closed | SessionState::Failed => true,
            };

            if should_remove {
                warn!(session_id = id, state = ?session.state, "Removing idle session");
                removed += 1;
                false
            } else {
                true
            }
        });

        if removed > 0 && self.config.enable_metrics {
            let mut metrics = self.metrics.write().await;
            metrics.sessions_timed_out += removed as u64;
        }

        if removed > 0 {
            info!(removed, "Cleaned up idle sessions");
        }

        removed
    }

    /// Get manager metrics
    pub async fn get_metrics(&self) -> ManagerMetrics {
        self.metrics.read().await.clone()
    }

    /// Get active session count
    pub async fn active_session_count(&self) -> usize {
        self.sessions.read().await.len()
    }
}

/// Session status (for IPC/gRPC)
#[derive(Debug, Clone)]
pub struct SessionStatus {
    pub id: SessionId,
    pub role: SessionRole,
    pub state: SessionState,
    pub age: Duration,
    pub idle_time: Duration,
    pub has_traffic_keys: bool,
    pub metrics: SessionMetrics,
}

/// Session errors
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Session not found")]
    SessionNotFound,
    
    #[error("Too many sessions")]
    TooManySessions,
    
    #[error("Invalid session state for operation")]
    InvalidState,
    
    #[error("Invalid role for operation")]
    InvalidRole,
    
    #[error("Handshake not initialized")]
    HandshakeNotInitialized,
    
    #[error("Handshake failed: {0}")]
    HandshakeFailed(String),
    
    #[error("Capability negotiation failed: {0}")]
    CapabilityNegotiationFailed(String),
    
    /// Unsupported required capability error
    ///
    /// Contains the capability ID for CLOSE 0x07 frame generation.
    /// Reference: spec/Capability_Negotiation_Policy_EN.md §4.2
    #[error("Unsupported required capability: 0x{0:08X}")]
    UnsupportedCapability(u32),
}

impl SessionError {
    /// Build CLOSE 0x07 frame for unsupported capability
    ///
    /// If this error is `UnsupportedCapability`, returns the CLOSE frame bytes.
    /// Otherwise returns None.
    pub fn to_close_frame(&self) -> Option<Vec<u8>> {
        match self {
            Self::UnsupportedCapability(cap_id) => {
                Some(nyx_stream::management::build_close_unsupported_cap(*cap_id))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_client_session() {
        let manager = SessionManager::new(SessionManagerConfig::default());
        let session_id = manager.create_client_session().await.unwrap();
        assert_eq!(session_id, 1);

        let status = manager.get_session_status(session_id).await.unwrap();
        assert_eq!(status.role, SessionRole::Client);
        assert_eq!(status.state, SessionState::Idle);
    }

    #[tokio::test]
    async fn test_create_server_session() {
        let manager = SessionManager::new(SessionManagerConfig::default());
        let session_id = manager.create_server_session().await.unwrap();
        assert_eq!(session_id, 1);

        let status = manager.get_session_status(session_id).await.unwrap();
        assert_eq!(status.role, SessionRole::Server);
        assert_eq!(status.state, SessionState::Idle);
    }

    #[tokio::test]
    async fn test_session_id_allocation() {
        let manager = SessionManager::new(SessionManagerConfig::default());
        
        let id1 = manager.create_client_session().await.unwrap();
        let id2 = manager.create_client_session().await.unwrap();
        let id3 = manager.create_server_session().await.unwrap();
        
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[tokio::test]
    async fn test_max_sessions_limit() {
        let config = SessionManagerConfig {
            max_sessions: 2,
            ..Default::default()
        };
        let manager = SessionManager::new(config);
        
        assert!(manager.create_client_session().await.is_ok());
        assert!(manager.create_client_session().await.is_ok());
        
        // Third should fail
        let result = manager.create_client_session().await;
        assert!(matches!(result, Err(SessionError::TooManySessions)));
    }

    #[tokio::test]
    async fn test_close_session() {
        let manager = SessionManager::new(SessionManagerConfig::default());
        let session_id = manager.create_client_session().await.unwrap();
        
        assert!(manager.get_session_status(session_id).await.is_some());
        
        manager.close_session(session_id).await.unwrap();
        
        assert!(manager.get_session_status(session_id).await.is_none());
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let manager = SessionManager::new(SessionManagerConfig::default());
        
        let _id1 = manager.create_client_session().await.unwrap();
        let _id2 = manager.create_server_session().await.unwrap();
        
        let metrics = manager.get_metrics().await;
        assert_eq!(metrics.total_sessions_created, 2);
    }

    #[tokio::test]
    async fn test_active_session_count() {
        let manager = SessionManager::new(SessionManagerConfig::default());
        
        assert_eq!(manager.active_session_count().await, 0);
        
        let id1 = manager.create_client_session().await.unwrap();
        assert_eq!(manager.active_session_count().await, 1);
        
        let _id2 = manager.create_client_session().await.unwrap();
        assert_eq!(manager.active_session_count().await, 2);
        
        manager.close_session(id1).await.unwrap();
        assert_eq!(manager.active_session_count().await, 1);
    }

    #[tokio::test]
    async fn test_cleanup_idle_sessions() {
        let config = SessionManagerConfig {
            handshake_timeout: Duration::from_millis(100),
            idle_timeout: Duration::from_millis(100),
            ..Default::default()
        };
        let manager = SessionManager::new(config);
        
        let _id = manager.create_client_session().await.unwrap();
        assert_eq!(manager.active_session_count().await, 1);
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        let removed = manager.cleanup_idle_sessions().await;
        assert_eq!(removed, 1);
        assert_eq!(manager.active_session_count().await, 0);
    }

    #[tokio::test]
    async fn test_unsupported_capability_error() {
        use nyx_stream::capability::{Capability, FLAG_REQUIRED};
        
        let manager = SessionManager::new(SessionManagerConfig::default());
        let session_id = manager.create_server_session().await.unwrap();
        
        // Create a capability that is not supported (arbitrary high ID)
        let unsupported_cap = Capability::new(0x99999999, FLAG_REQUIRED, vec![]);
        let client_caps = vec![unsupported_cap];
        
        // Attempt to process client hello with unsupported capability
        let result = manager
            .process_client_hello(session_id, &[0u8; 64], Some(client_caps))
            .await;
        
        // Should fail with UnsupportedCapability error
        assert!(matches!(
            result,
            Err(SessionError::UnsupportedCapability(0x99999999))
        ));
    }

    #[tokio::test]
    async fn test_unsupported_capability_close_frame() {
        let cap_id = 0x12345678u32;
        let error = SessionError::UnsupportedCapability(cap_id);
        
        // Should generate CLOSE frame
        let close_frame = error.to_close_frame().expect("Should return Some for UnsupportedCapability");
        
        // Verify frame structure: 2 bytes error code + 4 bytes capability ID
        assert_eq!(close_frame.len(), 6);
        
        // Verify error code (0x0007)
        let error_code = u16::from_be_bytes([close_frame[0], close_frame[1]]);
        assert_eq!(error_code, 0x0007);
        
        // Verify capability ID
        let parsed_cap_id = u32::from_be_bytes([
            close_frame[2],
            close_frame[3],
            close_frame[4],
            close_frame[5],
        ]);
        assert_eq!(parsed_cap_id, cap_id);
    }

    #[tokio::test]
    async fn test_other_errors_no_close_frame() {
        let error = SessionError::SessionNotFound;
        assert!(error.to_close_frame().is_none());
        
        let error = SessionError::HandshakeFailed("test".into());
        assert!(error.to_close_frame().is_none());
    }
}
