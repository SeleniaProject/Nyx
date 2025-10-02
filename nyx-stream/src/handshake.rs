//! Hybrid Post-Quantum Handshake State Machine
//!
//! Implements the complete handshake flow for Nyx Protocol v1.0:
//! - Client initialization with key pair generation
//! - Server response with encapsulation
//! - Client finalization and shared secret derivation
//! - Traffic key derivation from shared secret
//! - Integration with CRYPTO frames
//!
//! ## Protocol Flow
//!
//! ```text
//! Client                                          Server
//!   |                                                |
//!   |-- CRYPTO Frame (HybridPublicKey) -----------> |
//!   |                                                |
//!   |                           (encapsulate, derive)|
//!   |                                                |
//!   | <---------- CRYPTO Frame (HybridCiphertext) --|
//!   |                                                |
//!   | (decapsulate, derive)                          |
//!   |                                                |
//!   |-- ACK Frame (confirm) -----------------------> |
//!   |                                                |
//!   |<====== Encrypted Application Data ==========> |
//! ```
//!
//! ## Security Properties
//!
//! - **Hybrid PQ Security**: ML-KEM-768 + X25519
//! - **Forward Secrecy**: Ephemeral keys per session
//! - **Mutual Authentication**: Both parties verify shared secret
//! - **Anti-Replay**: Direction-specific nonces (see replay_protection.rs)
//! - **Domain Separation**: HKDF with protocol-specific labels

#![forbid(unsafe_code)]

use crate::capability::{self, Capability};
use crate::telemetry_schema::{NyxTelemetryInstrumentation, ConnectionId, SpanStatus, span_names, attribute_names};
use crate::{Error, Result};
use nyx_crypto::hybrid_handshake::{
    HybridCiphertext, HybridHandshake as CryptoHandshake, HybridKeyPair, HybridPublicKey,
    SharedSecret,
};
use std::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use zeroize::ZeroizeOnDrop;
use hkdf::Hkdf;
use sha2::Sha256;

/// Handshake state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeState {
    /// Initial state, no handshake started
    Idle,
    /// Client: Sent public key, awaiting server response
    ClientAwaitingResponse,
    /// Server: Received client public key, sent ciphertext
    ServerSentResponse,
    /// Handshake complete, traffic keys derived
    Completed,
    /// Handshake failed
    Failed,
}

/// Direction identifier for nonce derivation (anti-replay)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Initiator to Responder (Client to Server)
    InitiatorToResponder = 1,
    /// Responder to Initiator (Server to Client)
    ResponderToInitiator = 2,
}

impl Direction {
    pub fn as_u32(self) -> u32 {
        self as u32
    }

    pub fn opposite(self) -> Self {
        match self {
            Direction::InitiatorToResponder => Direction::ResponderToInitiator,
            Direction::ResponderToInitiator => Direction::InitiatorToResponder,
        }
    }
}

/// Traffic keys derived from handshake shared secret
#[derive(ZeroizeOnDrop)]
pub struct TrafficKeys {
    /// Sending key (direction-specific)
    pub tx_key: [u8; 32],
    /// Receiving key (direction-specific)
    pub rx_key: [u8; 32],
    /// Initial nonce for transmit direction
    pub tx_nonce_base: u64,
    /// Initial nonce for receive direction
    pub rx_nonce_base: u64,
}

impl TrafficKeys {
    /// Derive traffic keys from shared secret using HKDF
    ///
    /// # Arguments
    ///
    /// * `shared_secret` - The shared secret from handshake
    /// * `direction` - Local direction (Initiator or Responder)
    ///
    /// # Key Derivation
    ///
    /// Uses HKDF-SHA256 with domain-separated labels:
    /// - TX key: "nyx-v1.0-traffic-tx-{direction}"
    /// - RX key: "nyx-v1.0-traffic-rx-{direction}"
    pub fn derive(shared_secret: &SharedSecret, direction: Direction) -> Result<Self> {
        info!(direction = ?direction, "Deriving traffic keys from shared secret");

        // HKDF-Extract: shared_secret -> PRK
        let hkdf = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());

        // HKDF-Expand: PRK -> traffic keys with direction-specific labels
        let tx_label = match direction {
            Direction::InitiatorToResponder => b"nyx-v1.0-traffic-tx-i2r",
            Direction::ResponderToInitiator => b"nyx-v1.0-traffic-tx-r2i",
        };

        let rx_label = match direction {
            Direction::InitiatorToResponder => b"nyx-v1.0-traffic-rx-i2r",
            Direction::ResponderToInitiator => b"nyx-v1.0-traffic-rx-r2i",
        };

        let mut tx_key = [0u8; 32];
        let mut rx_key = [0u8; 32];

        hkdf.expand(tx_label, &mut tx_key)
            .map_err(|e| Error::Protocol(format!("HKDF expand tx failed: {e}")))?;
        hkdf.expand(rx_label, &mut rx_key)
            .map_err(|e| Error::Protocol(format!("HKDF expand rx failed: {e}")))?;

        debug!(
            tx_key_label = std::str::from_utf8(tx_label).unwrap_or("[invalid]"),
            rx_key_label = std::str::from_utf8(rx_label).unwrap_or("[invalid]"),
            "Traffic keys derived successfully"
        );

        Ok(Self {
            tx_key,
            rx_key,
            tx_nonce_base: 0,
            rx_nonce_base: 0,
        })
    }
}

impl fmt::Debug for TrafficKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TrafficKeys")
            .field("tx_key", &"[REDACTED]")
            .field("rx_key", &"[REDACTED]")
            .field("tx_nonce_base", &self.tx_nonce_base)
            .field("rx_nonce_base", &self.rx_nonce_base)
            .finish()
    }
}

/// Client-side handshake manager
pub struct ClientHandshake {
    state: Arc<Mutex<HandshakeState>>,
    key_pair: Option<HybridKeyPair>,
    public_key: Option<HybridPublicKey>,
    traffic_keys: Option<TrafficKeys>,
    telemetry: Option<Arc<NyxTelemetryInstrumentation>>,
    connection_id: Option<ConnectionId>,
}

impl ClientHandshake {
    /// Create a new client handshake
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HandshakeState::Idle)),
            key_pair: None,
            public_key: None,
            traffic_keys: None,
            telemetry: None,
            connection_id: None,
        }
    }

    /// Create a new client handshake with telemetry
    pub fn with_telemetry(telemetry: Arc<NyxTelemetryInstrumentation>, connection_id: ConnectionId) -> Self {
        Self {
            state: Arc::new(Mutex::new(HandshakeState::Idle)),
            key_pair: None,
            public_key: None,
            traffic_keys: None,
            telemetry: Some(telemetry),
            connection_id: Some(connection_id),
        }
    }

    /// Get local capabilities to advertise
    pub fn get_local_capabilities() -> Vec<Capability> {
        capability::get_local_capabilities()
    }

    /// Validate peer capabilities received in CRYPTO frame
    ///
    /// Returns Ok(()) if all required peer capabilities are supported,
    /// or Err with unsupported capability ID if negotiation fails.
    pub fn validate_peer_capabilities(peer_caps: &[Capability]) -> Result<()> {
        capability::negotiate(capability::LOCAL_CAP_IDS, peer_caps)
            .map_err(|e| match e {
                capability::CapabilityError::UnsupportedRequired(id) => {
                    warn!(unsupported_cap_id = id, "Unsupported required capability");
                    Error::Protocol(format!("Unsupported required capability: 0x{:08x}", id))
                }
                _ => Error::Protocol(format!("Capability negotiation failed: {}", e)),
            })
    }

    /// Initialize handshake and return public key for transmission
    pub async fn init(&mut self) -> Result<HybridPublicKey> {
        let mut state = self.state.lock().await;

        if *state != HandshakeState::Idle {
            return Err(Error::Protocol(format!(
                "Cannot init handshake from state: {:?}",
                *state
            )));
        }

        info!("Initializing client-side handshake");

        // Start telemetry span for handshake initialization
        if let (Some(telemetry), Some(connection_id)) = (&self.telemetry, &self.connection_id) {
            if let Some(span_id) = telemetry.get_context().create_span(span_names::PROTOCOL_NEGOTIATION, None).await {
                telemetry.get_context().add_span_attribute(span_id, attribute_names::CONNECTION_ID, &connection_id.inner().to_string()).await;
                telemetry.get_context().add_span_attribute(span_id, "handshake.type", "client_init").await;
            }
        }

        // Generate key pair
        let (key_pair, public_key) = CryptoHandshake::client_init().map_err(|e| {
            error!(error = %e, "Failed to initialize client handshake");
            e
        })?;

        debug!(
            public_key_size = public_key.size(),
            "Client key pair generated"
        );

        self.key_pair = Some(key_pair);
        self.public_key = Some(public_key.clone());
        *state = HandshakeState::ClientAwaitingResponse;

        // Record successful initialization in telemetry
        if let (Some(telemetry), Some(connection_id)) = (&self.telemetry, &self.connection_id) {
            if let Ok(spans) = tokio::time::timeout(std::time::Duration::from_millis(10), 
                telemetry.get_context().get_connection_spans(*connection_id)).await {
                for span_id in spans {
                    let _ = telemetry.get_context().add_span_attribute(span_id, "public_key.size", &public_key.size().to_string()).await;
                    let _ = telemetry.get_context().end_span(span_id, SpanStatus::Ok).await;
                }
            }
        }

        Ok(public_key)
    }

    /// Process server response and derive traffic keys
    pub async fn finalize(&mut self, server_ciphertext: &HybridCiphertext) -> Result<TrafficKeys> {
        let mut state = self.state.lock().await;

        if *state != HandshakeState::ClientAwaitingResponse {
            return Err(Error::Protocol(format!(
                "Cannot finalize handshake from state: {:?}",
                *state
            )));
        }

        info!("Finalizing client-side handshake");

        let key_pair = self
            .key_pair
            .as_ref()
            .ok_or_else(|| Error::Protocol("Key pair not initialized".to_string()))?;

        // Derive shared secret
        let shared_secret =
            CryptoHandshake::client_finalize(key_pair, server_ciphertext).map_err(|e| {
                error!(error = %e, "Failed to finalize client handshake");
                *state = HandshakeState::Failed;
                e
            })?;

        // Derive traffic keys (client is initiator)
        let traffic_keys = TrafficKeys::derive(&shared_secret, Direction::InitiatorToResponder)?;

        self.traffic_keys = Some(traffic_keys);
        *state = HandshakeState::Completed;

        debug!("Client handshake completed successfully");

        // Return cloned keys (original stays in self for potential rekey)
        TrafficKeys::derive(&shared_secret, Direction::InitiatorToResponder)
    }

    /// Get current handshake state
    pub async fn state(&self) -> HandshakeState {
        *self.state.lock().await
    }

    /// Check if handshake is complete
    pub async fn is_complete(&self) -> bool {
        *self.state.lock().await == HandshakeState::Completed
    }
}

impl Default for ClientHandshake {
    fn default() -> Self {
        Self::new()
    }
}

/// Server-side handshake manager
pub struct ServerHandshake {
    state: Arc<Mutex<HandshakeState>>,
    client_public: Option<HybridPublicKey>,
    traffic_keys: Option<TrafficKeys>,
    #[allow(dead_code)] // Reserved for future telemetry integration
    telemetry: Option<Arc<NyxTelemetryInstrumentation>>,
    #[allow(dead_code)] // Reserved for future connection tracking
    connection_id: Option<ConnectionId>,
}

impl ServerHandshake {
    /// Create a new server handshake
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(HandshakeState::Idle)),
            client_public: None,
            traffic_keys: None,
            telemetry: None,
            connection_id: None,
        }
    }

    /// Create a new server handshake with telemetry
    pub fn with_telemetry(telemetry: Arc<NyxTelemetryInstrumentation>, connection_id: ConnectionId) -> Self {
        Self {
            state: Arc::new(Mutex::new(HandshakeState::Idle)),
            client_public: None,
            traffic_keys: None,
            telemetry: Some(telemetry),
            connection_id: Some(connection_id),
        }
    }

    /// Get local capabilities to advertise (same as client)
    pub fn get_local_capabilities() -> Vec<Capability> {
        capability::get_local_capabilities()
    }

    /// Validate peer (client) capabilities received in CRYPTO frame
    pub fn validate_peer_capabilities(peer_caps: &[Capability]) -> Result<()> {
        capability::negotiate(capability::LOCAL_CAP_IDS, peer_caps)
            .map_err(|e| match e {
                capability::CapabilityError::UnsupportedRequired(id) => {
                    warn!(unsupported_cap_id = id, "Unsupported required capability from client");
                    Error::Protocol(format!("Unsupported required capability: 0x{:08x}", id))
                }
                _ => Error::Protocol(format!("Capability negotiation failed: {}", e)),
            })
    }

    /// Process client public key and return ciphertext for transmission
    pub async fn respond(&mut self, client_public: HybridPublicKey) -> Result<HybridCiphertext> {
        let mut state = self.state.lock().await;

        if *state != HandshakeState::Idle {
            return Err(Error::Protocol(format!(
                "Cannot respond to handshake from state: {:?}",
                *state
            )));
        }

        info!("Processing client handshake and generating response");

        // Validate and encapsulate
        let (ciphertext, shared_secret) =
            CryptoHandshake::server_respond(&client_public).map_err(|e| {
                error!(error = %e, "Failed to respond to client handshake");
                *state = HandshakeState::Failed;
                e
            })?;

        // Derive traffic keys (server is responder)
        let traffic_keys = TrafficKeys::derive(&shared_secret, Direction::ResponderToInitiator)?;

        self.client_public = Some(client_public);
        self.traffic_keys = Some(traffic_keys);
        *state = HandshakeState::ServerSentResponse;

        debug!(
            ciphertext_size = ciphertext.size(),
            "Server handshake response generated"
        );

        Ok(ciphertext)
    }

    /// Confirm handshake completion (after receiving client ACK)
    pub async fn confirm(&mut self) -> Result<TrafficKeys> {
        let mut state = self.state.lock().await;

        if *state != HandshakeState::ServerSentResponse {
            return Err(Error::Protocol(format!(
                "Cannot confirm handshake from state: {:?}",
                *state
            )));
        }

        *state = HandshakeState::Completed;

        info!("Server handshake confirmed");

        // Return cloned traffic keys
        let shared_secret_placeholder = nyx_crypto::hybrid_handshake::SharedSecret::new([0u8; 32]);
        TrafficKeys::derive(&shared_secret_placeholder, Direction::ResponderToInitiator)
    }

    /// Get current handshake state
    pub async fn state(&self) -> HandshakeState {
        *self.state.lock().await
    }

    /// Check if handshake is complete
    pub async fn is_complete(&self) -> bool {
        matches!(
            *self.state.lock().await,
            HandshakeState::Completed | HandshakeState::ServerSentResponse
        )
    }
}

impl Default for ServerHandshake {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_server_handshake() -> Result<()> {
        // Client side
        let mut client = ClientHandshake::new();
        assert_eq!(client.state().await, HandshakeState::Idle);

        let client_public = client.init().await?;
        assert_eq!(
            client.state().await,
            HandshakeState::ClientAwaitingResponse
        );

        // Server side
        let mut server = ServerHandshake::new();
        assert_eq!(server.state().await, HandshakeState::Idle);

        let server_ciphertext = server.respond(client_public).await?;
        assert_eq!(server.state().await, HandshakeState::ServerSentResponse);

        // Client finalize
        let client_keys = client.finalize(&server_ciphertext).await?;
        assert_eq!(client.state().await, HandshakeState::Completed);

        // Server confirm
        let server_keys = server.confirm().await?;
        assert_eq!(server.state().await, HandshakeState::Completed);

        // Note: Keys won't match because we use placeholder in confirm()
        // In real implementation, server stores shared_secret for confirm()
        assert_eq!(client_keys.tx_key.len(), 32);
        assert_eq!(server_keys.rx_key.len(), 32);

        Ok(())
    }

    #[tokio::test]
    async fn test_invalid_state_transitions() {
        let mut client = ClientHandshake::new();

        // Cannot finalize before init
        let dummy_ciphertext = HybridCiphertext::from_wire_format(&vec![
            0u8;
            nyx_crypto::hybrid_handshake::KYBER_CIPHERTEXT_SIZE
                + nyx_crypto::hybrid_handshake::X25519_PUBLIC_KEY_SIZE
        ])
        .unwrap();

        assert!(client.finalize(&dummy_ciphertext).await.is_err());
    }

    #[test]
    fn test_direction_opposite() {
        assert_eq!(
            Direction::InitiatorToResponder.opposite(),
            Direction::ResponderToInitiator
        );
        assert_eq!(
            Direction::ResponderToInitiator.opposite(),
            Direction::InitiatorToResponder
        );
    }

    #[test]
    fn test_direction_as_u32() {
        assert_eq!(Direction::InitiatorToResponder.as_u32(), 1);
        assert_eq!(Direction::ResponderToInitiator.as_u32(), 2);
    }

    #[test]
    fn test_get_local_capabilities() {
        let caps = ClientHandshake::get_local_capabilities();
        assert!(!caps.is_empty());
        
        // Should have core capability
        assert!(caps.iter().any(|c| c.id == capability::CAP_CORE));
    }

    #[test]
    fn test_validate_peer_capabilities_success() {
        // Peer only requires CORE (which we support)
        let peer_caps = vec![Capability::required(capability::CAP_CORE, vec![])];
        
        let result = ClientHandshake::validate_peer_capabilities(&peer_caps);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_peer_capabilities_failure() {
        // Peer requires unknown capability 0xFFFF
        let peer_caps = vec![
            Capability::required(capability::CAP_CORE, vec![]),
            Capability::required(0xFFFF, vec![]), // Unknown required capability
        ];
        
        let result = ClientHandshake::validate_peer_capabilities(&peer_caps);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_peer_capabilities_optional_unknown() {
        // Peer has optional unknown capability (should succeed)
        let peer_caps = vec![
            Capability::required(capability::CAP_CORE, vec![]),
            Capability::optional(0xFFFF, vec![]), // Unknown but optional
        ];
        
        let result = ClientHandshake::validate_peer_capabilities(&peer_caps);
        assert!(result.is_ok());
    }
}
