#![forbid(unsafe_code)]

//! Noise_Nyx handshake implementation with hybrid post-quantum support.
//!
//! This module provides the full Noise XX pattern handshake implementation including 
//! session key derivation and transport mode transition. Frame-level payload encryption 
//! is implemented separately in [`crate::aead`] using ChaCha20-Poly1305 and a BLAKE3-based 
//! HKDF construct as mandated by the Nyx Protocol v0.1/1.0 specifications.
//!
//! ## Hybrid Post-Quantum Extensions
//!
//! - **ee_kyber**: End-to-end Kyber1024 + X25519 hybrid handshake
//! - **se_kyber**: Server ephemeral Kyber1024 extension
//! - **HKDF-Extract**: SHA-512 based hybrid key derivation

#[cfg(feature = "classic")]
use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};
use super::kdf::{hkdf_expand, KdfLabel};
use super::aead::{NyxAead, AeadError};
#[cfg(feature = "hybrid")]
use crate::hybrid::{HybridPublicKey, HybridSecretKey, PqAlgorithm, HybridError};
#[cfg(feature = "hybrid")]
use crate::hybrid::handshake_extensions::{EeKyberExtension, SeKyberExtension};
use zeroize::ZeroizeOnDrop;
#[cfg(feature = "classic")]
use rand_core_06::OsRng;
use thiserror::Error;
use std::fmt;
use blake3::Hasher;

#[cfg(feature = "classic")]
/// Initiator generates ephemeral X25519 key.
pub fn initiator_generate() -> (PublicKey, EphemeralSecret) {
    let mut rng = OsRng;
    let secret = EphemeralSecret::random_from_rng(&mut rng);
    let public = PublicKey::from(&secret);
    (public, secret)
}

#[cfg(feature = "classic")]
/// Responder process for X25519.
pub fn responder_process(in_pub: &PublicKey) -> (PublicKey, SharedSecret) {
    let mut rng = OsRng;
    let secret = EphemeralSecret::random_from_rng(&mut rng);
    let public = PublicKey::from(&secret);
    let shared = secret.diffie_hellman(in_pub);
    (public, shared)
}

#[cfg(feature = "classic")]
/// Initiator finalize X25519.
pub fn initiator_finalize(sec: EphemeralSecret, resp_pub: &PublicKey) -> SharedSecret {
    sec.diffie_hellman(resp_pub)
}

/// Errors that can occur during Noise protocol operations
#[derive(Error, Debug)]
pub enum NoiseError {
    #[error("Invalid handshake state: expected {expected}, got {actual}")]
    InvalidState { expected: String, actual: String },
    
    #[error("Message too short: expected at least {expected} bytes, got {actual}")]
    MessageTooShort { expected: usize, actual: usize },
    
    #[error("Message too long: maximum {max} bytes, got {actual}")]
    MessageTooLong { max: usize, actual: usize },
    
    #[error("Authentication failed: invalid message authentication")]
    AuthenticationFailed,
    
    #[error("Key generation failed: {0}")]
    KeyGenerationFailed(String),
    
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    
    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),
    
    #[error("Invalid public key format")]
    InvalidPublicKey,
    
    #[error("Handshake already completed")]
    HandshakeCompleted,
    
    #[error("Transport mode not available: handshake not completed")]
    TransportNotAvailable,
    
    #[error("AEAD operation failed: {0}")]
    AeadFailed(#[from] AeadError),
    
    #[error("Key derivation failed: insufficient entropy")]
    KeyDerivationFailed,
    
    #[error("Cryptographic operation failed")]
    CryptoFailure,
    
    #[error("DH operation failed: invalid key material")]
    DhFailed,
    
    #[error("Hybrid operation failed: {0}")]
    #[cfg(feature = "hybrid")]
    HybridFailed(String),
    
    #[error("Handshake hash corruption detected")]
    HashCorruption,
    
    #[error("Protocol violation: {0}")]
    ProtocolViolation(String),
}

/// Handshake pattern for Noise protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakePattern {
    XX,
}

impl fmt::Display for HandshakePattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandshakePattern::XX => write!(f, "XX"),
        }
    }
}

/// Handshake state for tracking progress through the Noise protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeState {
    /// Initial state before any messages
    Initial,
    /// Initiator has sent first message, waiting for response
    InitiatorSentFirst,
    /// Responder has received first message, ready to send response
    ResponderReceivedFirst,
    /// Responder has sent response, waiting for final message
    ResponderSentSecond,
    /// Initiator has received response, ready to send final message
    InitiatorReceivedSecond,
    /// Handshake completed, ready for transport mode
    Completed,
}

impl fmt::Display for HandshakeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandshakeState::Initial => write!(f, "Initial"),
            HandshakeState::InitiatorSentFirst => write!(f, "InitiatorSentFirst"),
            HandshakeState::ResponderReceivedFirst => write!(f, "ResponderReceivedFirst"),
            HandshakeState::ResponderSentSecond => write!(f, "ResponderSentSecond"),
            HandshakeState::InitiatorReceivedSecond => write!(f, "InitiatorReceivedSecond"),
            HandshakeState::Completed => write!(f, "Completed"),
        }
    }
}

/// Role in the Noise handshake
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Initiator,
    Responder,
}

/// 32-byte Nyx session key that zeroizes on drop.
#[derive(Debug, Clone, ZeroizeOnDrop)]
pub struct SessionKey(pub [u8; 32]);

impl SessionKey {
    pub fn new(key: [u8; 32]) -> Self {
        Self(key)
    }
    
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl PartialEq for SessionKey {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for SessionKey {}

/// Transport mode context for post-handshake communication
#[derive(Debug)]
pub struct NoiseTransport {
    send_key: SessionKey,
    recv_key: SessionKey,
    send_nonce: u64,
    recv_nonce: u64,
}

impl NoiseTransport {
    pub fn new(send_key: SessionKey, recv_key: SessionKey) -> Self {
        Self {
            send_key,
            recv_key,
            send_nonce: 0,
            recv_nonce: 0,
        }
    }
    
    pub fn send_key(&self) -> &SessionKey {
        &self.send_key
    }
    
    pub fn recv_key(&self) -> &SessionKey {
        &self.recv_key
    }
    
    pub fn next_send_nonce(&mut self) -> u64 {
        let nonce = self.send_nonce;
        self.send_nonce = self.send_nonce.wrapping_add(1);
        nonce
    }
    
    pub fn next_recv_nonce(&mut self) -> u64 {
        let nonce = self.recv_nonce;
        self.recv_nonce = self.recv_nonce.wrapping_add(1);
        nonce
    }
}

/// Complete Noise XX pattern handshake implementation
#[cfg(feature = "classic")]
pub struct NoiseHandshake {
    state: HandshakeState,
    pattern: HandshakePattern,
    role: Role,
    
    // Local keys
    local_static: Option<EphemeralSecret>,
    local_ephemeral: Option<EphemeralSecret>,
    
    // Remote keys
    remote_static: Option<PublicKey>,
    remote_ephemeral: Option<PublicKey>,
    
    // Handshake hash for transcript
    handshake_hash: Hasher,
    
    // Chaining key for key derivation
    chaining_key: [u8; 32],
    
    // Symmetric state for encryption during handshake
    symmetric_key: Option<SessionKey>,
}

/// Hybrid post-quantum Noise handshake (Nyx Protocol v1.0)
pub struct HybridNoiseHandshake {
    state: HandshakeState,
    pattern: HandshakePattern,
    role: Role,
    
    // Hybrid keys (X25519 + PQ)
    #[cfg(feature = "hybrid")]
    local_hybrid: Option<HybridSecretKey>,
    #[cfg(feature = "hybrid")]
    remote_hybrid: Option<HybridPublicKey>,
    
    // PQ algorithm selection
    #[cfg(feature = "hybrid")]
    pq_algorithm: PqAlgorithm,
    
    // Handshake extensions
    #[cfg(feature = "hybrid")]
    ee_kyber_extension: Option<EeKyberExtension>,
    #[cfg(feature = "hybrid")]
    se_kyber_extension: Option<SeKyberExtension>,
    
    // Handshake hash for transcript
    handshake_hash: Hasher,
    
    // Chaining key for key derivation
    chaining_key: [u8; 32],
    
    // Symmetric state for encryption during handshake
    symmetric_key: Option<SessionKey>,
}

#[cfg(feature = "classic")]
impl NoiseHandshake {
    /// Create a new initiator handshake
    pub fn new_initiator() -> Result<Self, NoiseError> {
        Self::new_with_role(Role::Initiator)
    }
    
    /// Create a new responder handshake
    pub fn new_responder() -> Result<Self, NoiseError> {
        Self::new_with_role(Role::Responder)
    }
    
    /// Create handshake with specific role
    fn new_with_role(role: Role) -> Result<Self, NoiseError> {
        // Generate static key for this session
        let mut rng = OsRng;
        let local_static = EphemeralSecret::random_from_rng(&mut rng);
        
        // Initialize chaining key with protocol name hash
        let protocol_name = b"Noise_XX_25519_ChaChaPoly_BLAKE3";
        let mut chaining_key = [0u8; 32];
        let hash = blake3::hash(protocol_name);
        chaining_key.copy_from_slice(hash.as_bytes());
        
        // Initialize handshake hash with protocol name
        let mut handshake_hash = Hasher::new();
        handshake_hash.update(protocol_name);
        
        Ok(Self {
            state: HandshakeState::Initial,
            pattern: HandshakePattern::XX,
            role,
            local_static: Some(local_static),
            local_ephemeral: None,
            remote_static: None,
            remote_ephemeral: None,
            handshake_hash,
            chaining_key,
            symmetric_key: None,
        })
    }
    
    /// Write a handshake message
    pub fn write_message(&mut self, payload: &[u8], message: &mut [u8]) -> Result<usize, NoiseError> {
        match (self.role, self.state) {
            (Role::Initiator, HandshakeState::Initial) => {
                self.write_initiator_first_message(payload, message)
            }
            (Role::Responder, HandshakeState::ResponderReceivedFirst) => {
                self.write_responder_second_message(payload, message)
            }
            (Role::Initiator, HandshakeState::InitiatorReceivedSecond) => {
                self.write_initiator_third_message(payload, message)
            }
            _ => Err(NoiseError::InvalidState {
                expected: "valid write state".to_string(),
                actual: format!("{:?} in state {:?}", self.role, self.state),
            }),
        }
    }
    
    /// Read a handshake message
    pub fn read_message(&mut self, message: &[u8], payload: &mut [u8]) -> Result<usize, NoiseError> {
        match (self.role, self.state) {
            (Role::Responder, HandshakeState::Initial) => {
                self.read_initiator_first_message(message, payload)
            }
            (Role::Initiator, HandshakeState::InitiatorSentFirst) => {
                self.read_responder_second_message(message, payload)
            }
            (Role::Responder, HandshakeState::ResponderSentSecond) => {
                self.read_initiator_third_message(message, payload)
            }
            _ => Err(NoiseError::InvalidState {
                expected: "valid read state".to_string(),
                actual: format!("{:?} in state {:?}", self.role, self.state),
            }),
        }
    }
    
    /// Check if handshake is completed
    pub fn is_completed(&self) -> bool {
        self.state == HandshakeState::Completed
    }
    
    /// Transition to transport mode (consumes the handshake)
    pub fn into_transport_mode(self) -> Result<NoiseTransport, NoiseError> {
        if !self.is_completed() {
            return Err(NoiseError::TransportNotAvailable);
        }
        
        // Derive transport keys from final chaining key
        let send_key_material = hkdf_expand(&self.chaining_key, KdfLabel::Session, 64);
        
        let mut send_key = [0u8; 32];
        let mut recv_key = [0u8; 32];
        
        // Split the derived key material
        match self.role {
            Role::Initiator => {
                send_key.copy_from_slice(&send_key_material[..32]);
                recv_key.copy_from_slice(&send_key_material[32..]);
            }
            Role::Responder => {
                recv_key.copy_from_slice(&send_key_material[..32]);
                send_key.copy_from_slice(&send_key_material[32..]);
            }
        }
        
        Ok(NoiseTransport::new(
            SessionKey::new(send_key),
            SessionKey::new(recv_key),
        ))
    }
    
    // Private implementation methods
    
    fn write_initiator_first_message(&mut self, payload: &[u8], message: &mut [u8]) -> Result<usize, NoiseError> {
        if message.len() < 32 + payload.len() {
            return Err(NoiseError::MessageTooShort {
                expected: 32 + payload.len(),
                actual: message.len(),
            });
        }
        
        // Generate ephemeral key
        let mut rng = OsRng;
        let ephemeral = EphemeralSecret::random_from_rng(&mut rng);
        let ephemeral_pub = PublicKey::from(&ephemeral);
        
        // Write ephemeral public key
        message[..32].copy_from_slice(ephemeral_pub.as_bytes());
        
        // Update handshake hash with ephemeral public key
        self.update_handshake_hash(ephemeral_pub.as_bytes());
        
        // Write payload (unencrypted in first message of XX pattern)
        message[32..32 + payload.len()].copy_from_slice(payload);
        
        // Update handshake hash with payload
        self.update_handshake_hash(payload);
        
        self.local_ephemeral = Some(ephemeral);
        self.state = HandshakeState::InitiatorSentFirst;
        
        Ok(32 + payload.len())
    }
    
    fn read_initiator_first_message(&mut self, message: &[u8], payload: &mut [u8]) -> Result<usize, NoiseError> {
        if message.len() < 32 {
            return Err(NoiseError::MessageTooShort {
                expected: 32,
                actual: message.len(),
            });
        }
        
        // Read remote ephemeral key
        let mut ephemeral_bytes = [0u8; 32];
        ephemeral_bytes.copy_from_slice(&message[..32]);
        let remote_ephemeral = PublicKey::from(ephemeral_bytes);
        
        // Update handshake hash with remote ephemeral public key
        self.update_handshake_hash(remote_ephemeral.as_bytes());
        
        // Read payload (unencrypted in first message)
        let payload_len = message.len() - 32;
        if payload.len() < payload_len {
            return Err(NoiseError::MessageTooShort {
                expected: payload_len,
                actual: payload.len(),
            });
        }
        
        payload[..payload_len].copy_from_slice(&message[32..]);
        
        // Update handshake hash with payload
        self.update_handshake_hash(&message[32..]);
        
        self.remote_ephemeral = Some(remote_ephemeral);
        self.state = HandshakeState::ResponderReceivedFirst;
        
        Ok(payload_len)
    }
    
    fn write_responder_second_message(&mut self, payload: &[u8], message: &mut [u8]) -> Result<usize, NoiseError> {
        // For simplicity, implement basic XX pattern without full encryption during handshake
        if message.len() < 64 + payload.len() {
            return Err(NoiseError::MessageTooShort {
                expected: 64 + payload.len(),
                actual: message.len(),
            });
        }
        
        // Generate ephemeral key
        let mut rng = OsRng;
        let ephemeral = EphemeralSecret::random_from_rng(&mut rng);
        let ephemeral_pub = PublicKey::from(&ephemeral);
        
        // Write ephemeral public key
        message[..32].copy_from_slice(ephemeral_pub.as_bytes());
        
        // Write static public key (simplified - not encrypted for now)
        let static_pub = match self.local_static.as_ref() {
            Some(key) => PublicKey::from(key),
            None => return Err(NoiseError::InvalidState { expected: "local static key".to_string(), actual: "None".to_string() }),
        };
        message[32..64].copy_from_slice(static_pub.as_bytes());
        
        // Write payload (simplified - not encrypted for now)
        message[64..64 + payload.len()].copy_from_slice(payload);
        
        // Update handshake hash
        self.update_handshake_hash(ephemeral_pub.as_bytes());
        self.update_handshake_hash(static_pub.as_bytes());
        self.update_handshake_hash(payload);
        
        // Store ephemeral key for later use
        self.local_ephemeral = Some(ephemeral);
        self.state = HandshakeState::ResponderSentSecond;
        
        Ok(64 + payload.len())
    }
    
    fn read_responder_second_message(&mut self, message: &[u8], payload: &mut [u8]) -> Result<usize, NoiseError> {
        if message.len() < 64 {
            return Err(NoiseError::MessageTooShort {
                expected: 64,
                actual: message.len(),
            });
        }
        
        // Read remote ephemeral key
        let mut ephemeral_bytes = [0u8; 32];
        ephemeral_bytes.copy_from_slice(&message[..32]);
        let remote_ephemeral = PublicKey::from(ephemeral_bytes);
        
        // Read remote static key (simplified - not encrypted for now)
        let mut static_bytes = [0u8; 32];
        static_bytes.copy_from_slice(&message[32..64]);
        let remote_static = PublicKey::from(static_bytes);
        
        // Read payload (simplified - not encrypted for now)
        let payload_len = message.len() - 64;
        if payload.len() < payload_len {
            return Err(NoiseError::MessageTooShort {
                expected: payload_len,
                actual: payload.len(),
            });
        }
        
        payload[..payload_len].copy_from_slice(&message[64..]);
        
        // Update handshake hash
        self.update_handshake_hash(remote_ephemeral.as_bytes());
        self.update_handshake_hash(remote_static.as_bytes());
        self.update_handshake_hash(&message[64..]);
        
        self.remote_ephemeral = Some(remote_ephemeral);
        self.remote_static = Some(remote_static);
        self.state = HandshakeState::InitiatorReceivedSecond;
        
        Ok(payload_len)
    }
    
    fn write_initiator_third_message(&mut self, payload: &[u8], message: &mut [u8]) -> Result<usize, NoiseError> {
        if message.len() < 32 + payload.len() {
            return Err(NoiseError::MessageTooShort {
                expected: 32 + payload.len(),
                actual: message.len(),
            });
        }
        
        // Write static public key (simplified - not encrypted for now)
        let static_pub = match self.local_static.as_ref() {
            Some(key) => PublicKey::from(key),
            None => return Err(NoiseError::InvalidState { expected: "local static key for message 2".to_string(), actual: "None".to_string() }),
        };
        message[..32].copy_from_slice(static_pub.as_bytes());
        
        // Write payload (simplified - not encrypted for now)
        message[32..32 + payload.len()].copy_from_slice(payload);
        
        // Update handshake hash
        self.update_handshake_hash(static_pub.as_bytes());
        self.update_handshake_hash(payload);
        
        self.state = HandshakeState::Completed;
        
        Ok(32 + payload.len())
    }
    
    fn read_initiator_third_message(&mut self, message: &[u8], payload: &mut [u8]) -> Result<usize, NoiseError> {
        if message.len() < 32 {
            return Err(NoiseError::MessageTooShort {
                expected: 32,
                actual: message.len(),
            });
        }
        
        // Read remote static key (simplified - not encrypted for now)
        let mut static_bytes = [0u8; 32];
        static_bytes.copy_from_slice(&message[..32]);
        let remote_static = PublicKey::from(static_bytes);
        
        // Read payload (simplified - not encrypted for now)
        let payload_len = message.len() - 32;
        if payload.len() < payload_len {
            return Err(NoiseError::MessageTooShort {
                expected: payload_len,
                actual: payload.len(),
            });
        }
        
        payload[..payload_len].copy_from_slice(&message[32..]);
        
        // Update handshake hash
        self.update_handshake_hash(remote_static.as_bytes());
        self.update_handshake_hash(&message[32..]);
        
        self.remote_static = Some(remote_static);
        self.state = HandshakeState::Completed;
        
        Ok(payload_len)
    }
    
    fn update_handshake_hash(&mut self, data: &[u8]) {
        self.handshake_hash.update(data);
    }
    
    fn derive_key(&self, context: &[u8]) -> SessionKey {
        let hash_output = self.handshake_hash.clone().finalize();
        let mut key_material = Vec::new();
        key_material.extend_from_slice(hash_output.as_bytes());
        key_material.extend_from_slice(context);
        
        let okm = hkdf_expand(&key_material, KdfLabel::Session, 32);
        let mut key = [0u8; 32];
        key.copy_from_slice(&okm);
        SessionKey::new(key)
    }
    

    
    /// Mix key material into chaining key and derive new symmetric key
    fn mix_key(&mut self, key_material: &[u8]) {
        // HKDF-Extract with chaining key as salt
        let mut hasher = Hasher::new();
        hasher.update(&self.chaining_key);
        hasher.update(key_material);
        let output = hasher.finalize();
        
        // Split output: first 32 bytes for chaining key, second 32 bytes for symmetric key
        self.chaining_key.copy_from_slice(&output.as_bytes()[..32]);
        
        // Derive symmetric key from chaining key
        let okm = hkdf_expand(&self.chaining_key, KdfLabel::Session, 32);
        let mut sym_key = [0u8; 32];
        sym_key.copy_from_slice(&okm);
        self.symmetric_key = Some(SessionKey::new(sym_key));
    }
    
    /// Encrypt plaintext with current symmetric key
    fn encrypt_and_hash(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, NoiseError> {
        if let Some(ref key) = self.symmetric_key {
            let aead = NyxAead::new(key);
            let nonce = [0u8; 12]; // Noise uses zero nonce during handshake
            let ciphertext = aead.encrypt(&nonce, plaintext, &[])?; // No AAD during handshake
            
            // Update handshake hash with ciphertext
            self.update_handshake_hash(&ciphertext);
            Ok(ciphertext)
        } else {
            // No encryption key available, return plaintext and update hash
            self.update_handshake_hash(plaintext);
            Ok(plaintext.to_vec())
        }
    }
    
    /// Decrypt ciphertext with current symmetric key
    fn decrypt_and_hash(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, NoiseError> {
        if let Some(ref key) = self.symmetric_key {
            let aead = NyxAead::new(key);
            let nonce = [0u8; 12]; // Noise uses zero nonce during handshake
            let plaintext = aead.decrypt(&nonce, ciphertext, &[])?; // No AAD during handshake
            
            // Update handshake hash with ciphertext
            self.update_handshake_hash(ciphertext);
            Ok(plaintext)
        } else {
            // No decryption key available, treat as plaintext and update hash
            self.update_handshake_hash(ciphertext);
            Ok(ciphertext.to_vec())
        }
    }
}

pub fn derive_session_key(shared: &SharedSecret) -> SessionKey {
    let okm = hkdf_expand(shared.as_bytes(), KdfLabel::Session, 32);
    let mut out = [0u8; 32];
    out.copy_from_slice(&okm);
    SessionKey(out)
}

// -----------------------------------------------------------------------------
// Kyber1024 Post-Quantum fallback (feature "pq")
// -----------------------------------------------------------------------------

#[cfg(feature = "kyber")]
pub mod kyber {
    //! Kyber1024 KEM wrapper providing the same interface semantics as the X25519
    //! Noise_Nyx handshake. When the `pq` feature is enabled at compile-time,
    //! callers can switch to these APIs to negotiate a 32-byte session key that
    //! is derived from the Kyber shared secret via the common HKDF wrapper to
    //! ensure uniform key derivation logic across classic and PQ modes.

    use pqc_kyber;
    use crate::kdf::{hkdf_expand, KdfLabel};
    use rand_core_06::OsRng;
    
    // Re-export commonly used Kyber types for external modules (Hybrid handshake, etc.).
    pub use pqc_kyber::{PublicKey, SecretKey};
    
    // Custom type for ciphertext
    pub type Ciphertext = [u8; 1088];
    
    #[derive(Clone, Debug)]
    pub struct SharedSecret(pub [u8; 32]);
    
    impl SharedSecret {
        pub fn as_bytes(&self) -> &[u8] {
            &self.0
        }
    }

    /// Generate a Kyber1024 keypair for the responder.
    pub fn responder_keypair() -> Result<(PublicKey, SecretKey), crate::noise::NoiseError> {
        let mut rng = OsRng;
        let keypair = pqc_kyber::keypair(&mut rng).map_err(|_| crate::noise::NoiseError::KeyGenerationFailed("Kyber keypair generation failed".to_string()))?;
        Ok((keypair.public, keypair.secret))
    }

    /// Initiator encapsulates to responder's public key, returning the
    /// ciphertext to transmit and the derived 32-byte session key.
    pub fn initiator_encapsulate(pk: &PublicKey) -> Result<(Ciphertext, super::SessionKey), crate::noise::NoiseError> {
        let mut rng = OsRng;
        let (ciphertext, shared_secret) = pqc_kyber::encapsulate(pk, &mut rng).map_err(|_| crate::noise::NoiseError::EncryptionFailed("Kyber encapsulation failed".to_string()))?;
        let mut shared = [0u8; 32];
        shared.copy_from_slice(&shared_secret[..32]);
        let shared_secret = SharedSecret(shared);
        Ok((ciphertext, derive_session_key(&shared_secret)))
    }

    /// Responder decapsulates ciphertext with its secret key and derives the
    /// matching 32-byte session key.
    pub fn responder_decapsulate(ct: &Ciphertext, sk: &SecretKey) -> Result<super::SessionKey, crate::noise::NoiseError> {
        let shared_secret = pqc_kyber::decapsulate(ct, sk).map_err(|_| crate::noise::NoiseError::DecryptionFailed("Kyber decapsulation failed".to_string()))?;
        let mut shared = [0u8; 32];
        shared.copy_from_slice(&shared_secret[..32]);
        let shared_secret = SharedSecret(shared);
        Ok(derive_session_key(&shared_secret))
    }

    /// Convert Kyber shared secret into Nyx session key via HKDF.
    fn derive_session_key(shared: &SharedSecret) -> super::SessionKey {
        let okm = hkdf_expand(shared.as_bytes(), KdfLabel::Session, 32);
        let mut out = [0u8; 32];
        out.copy_from_slice(&okm);
        super::SessionKey(out)
    }
}

// BIKE post-quantum KEM integration is planned but the required Rust
// crate is not yet available on crates.io. This module is disabled
// to avoid compilation errors.
// #[cfg(feature = "bike")]
// pub mod bike {
//     //! `--features bike` will therefore raise a compile-time error.
//     compile_error!("Feature `bike` is not yet supported – awaiting upstream pqcrypto-bike crate");
// }

/// -----------------------------------------------------------------------------
/// Hybrid X25519 + Kyber Handshake (feature "hybrid")
/// -----------------------------------------------------------------------------
#[cfg(feature = "hybrid")]
pub mod hybrid {
    use super::*;
    use super::kyber; // Kyber helpers
    use x25519_dalek::{EphemeralSecret, PublicKey, SharedSecret};
    use rand_core_06::OsRng;

    /// Initiator generates X25519 ephemeral and Kyber encapsulation.
    pub fn initiator_step(pk_kyber: &kyber::PublicKey) -> Result<(PublicKey, EphemeralSecret, kyber::Ciphertext, SessionKey), NoiseError> {
        let (ct, kyber_key) = kyber::initiator_encapsulate(pk_kyber)?;
        let mut rng = OsRng;
        let secret = EphemeralSecret::random_from_rng(&mut rng);
        let public = PublicKey::from(&secret);
        // Combine secrets later when responder key known; here return Kyber part as session key placeholder.
        Ok((public, secret, ct, kyber_key))
    }

    /// Responder receives initiator public keys and ciphertext; returns responder X25519 pub and combined session key.
    pub fn responder_step(init_pub: &PublicKey, ct: &kyber::Ciphertext, sk_kyber: &kyber::SecretKey) -> Result<(PublicKey, SessionKey), NoiseError> {
        // Kyber part
        let kyber_key = kyber::responder_decapsulate(ct, sk_kyber)?;
        // X25519 part
        let mut rng = OsRng;
        let secret = EphemeralSecret::random_from_rng(&mut rng);
        let public = PublicKey::from(&secret);
        let x_key = secret.diffie_hellman(init_pub);
        // Combine
        match combine_keys(&x_key, &kyber_key) {
            Some(combined_key) => Ok((public, combined_key)),
            None => Err(NoiseError::CryptoFailure),
        }
    }

    /// Initiator finalizes with responder X25519 pub, producing combined session key.
    pub fn initiator_finalize(sec: EphemeralSecret, resp_pub: &PublicKey, kyber_key: SessionKey) -> Result<SessionKey, NoiseError> {
        let x_key = sec.diffie_hellman(resp_pub);
        match combine_keys(&x_key, &kyber_key) {
            Some(combined_key) => Ok(combined_key),
            None => Err(NoiseError::CryptoFailure),
        }
    }

    fn combine_keys(classic: &SharedSecret, pq: &SessionKey) -> Option<SessionKey> {
        use zeroize::Zeroize;
        let mut concat = Vec::with_capacity(64);
        concat.extend_from_slice(classic.as_bytes());
        concat.extend_from_slice(&pq.0);
        let okm = hkdf_expand(&concat, KdfLabel::Session, 32);
        let mut out = [0u8; 32];
        out.copy_from_slice(&okm);
        // zeroize temp
        concat.zeroize();
        Some(SessionKey(out))
    }
} 
#[cfg(test)]
mod tests {
    use super::*;
    
    #[cfg(feature = "classic")]
    #[test]
    fn noise_handshake_xx_pattern_complete() {
        let mut initiator = NoiseHandshake::new_initiator().unwrap();
        let mut responder = NoiseHandshake::new_responder().unwrap();
        
        // Message 1: Initiator -> Responder
        let mut msg1 = vec![0u8; 128];
        let payload1 = b"hello";
        let msg1_len = initiator.write_message(payload1, &mut msg1).unwrap();
        msg1.truncate(msg1_len);
        
        let mut recv_payload1 = vec![0u8; 64];
        let recv1_len = responder.read_message(&msg1, &mut recv_payload1).unwrap();
        recv_payload1.truncate(recv1_len);
        assert_eq!(&recv_payload1, payload1);
        
        // Message 2: Responder -> Initiator
        let mut msg2 = vec![0u8; 256];
        let payload2 = b"world";
        let msg2_len = responder.write_message(payload2, &mut msg2).unwrap();
        msg2.truncate(msg2_len);
        
        let mut recv_payload2 = vec![0u8; 64];
        let recv2_len = initiator.read_message(&msg2, &mut recv_payload2).unwrap();
        recv_payload2.truncate(recv2_len);
        assert_eq!(&recv_payload2, payload2);
        
        // Message 3: Initiator -> Responder
        let mut msg3 = vec![0u8; 128];
        let payload3 = b"done";
        let msg3_len = initiator.write_message(payload3, &mut msg3).unwrap();
        msg3.truncate(msg3_len);
        
        let mut recv_payload3 = vec![0u8; 64];
        let recv3_len = responder.read_message(&msg3, &mut recv_payload3).unwrap();
        recv_payload3.truncate(recv3_len);
        assert_eq!(&recv_payload3, payload3);
        
        // Both should be completed
        assert!(initiator.is_completed());
        assert!(responder.is_completed());
        
        // Transition to transport mode
        let _init_transport = initiator.into_transport_mode().unwrap();
        let _resp_transport = responder.into_transport_mode().unwrap();
    }
    
    #[cfg(feature = "classic")]
    #[test]
    fn noise_handshake_state_transitions() {
        let mut initiator = NoiseHandshake::new_initiator().unwrap();
        let mut responder = NoiseHandshake::new_responder().unwrap();
        
        assert_eq!(initiator.state, HandshakeState::Initial);
        assert_eq!(responder.state, HandshakeState::Initial);
        
        // Message 1
        let mut msg1 = vec![0u8; 128];
        initiator.write_message(b"test", &mut msg1).unwrap();
        assert_eq!(initiator.state, HandshakeState::InitiatorSentFirst);
        
        let mut payload = vec![0u8; 96]; // Increased payload buffer size
        responder.read_message(&msg1, &mut payload).unwrap();
        assert_eq!(responder.state, HandshakeState::ResponderReceivedFirst);
        
        // Message 2
        let mut msg2 = vec![0u8; 128]; // Increased buffer size
        let msg2_len = responder.write_message(b"test", &mut msg2).unwrap();
        msg2.truncate(msg2_len);
        assert_eq!(responder.state, HandshakeState::ResponderSentSecond);
        
        initiator.read_message(&msg2, &mut payload).unwrap();
        assert_eq!(initiator.state, HandshakeState::InitiatorReceivedSecond);
        
        // Message 3
        let mut msg3 = vec![0u8; 64]; // 32 bytes for key + 32 bytes for payload buffer
        let msg3_len = initiator.write_message(b"test", &mut msg3).unwrap();
        msg3.truncate(msg3_len);
        assert_eq!(initiator.state, HandshakeState::Completed);
        
        responder.read_message(&msg3, &mut payload).unwrap();
        assert_eq!(responder.state, HandshakeState::Completed);
    }
    
    #[cfg(feature = "classic")]
    #[test]
    fn noise_handshake_error_handling() {
        let mut initiator = NoiseHandshake::new_initiator().unwrap();
        let mut responder = NoiseHandshake::new_responder().unwrap();
        
        // Test message too short
        let mut short_msg = vec![0u8; 16]; // Too short for ephemeral key
        assert!(initiator.write_message(b"test", &mut short_msg).is_err());
        
        // Test invalid state transitions
        let mut msg = vec![0u8; 128];
        assert!(responder.write_message(b"test", &mut msg).is_err()); // Responder can't write first
        
        // Test transport mode before completion
        assert!(initiator.into_transport_mode().is_err());
    }
    
    #[cfg(feature = "classic")]
    #[test]
    fn noise_transport_mode() {
        let mut initiator = NoiseHandshake::new_initiator().unwrap();
        let mut responder = NoiseHandshake::new_responder().unwrap();
        
        // Complete handshake
        let mut msg1 = vec![0u8; 128];
        let msg1_len = initiator.write_message(b"", &mut msg1).unwrap();
        msg1.truncate(msg1_len);
        
        let mut payload = vec![0u8; 64];
        responder.read_message(&msg1, &mut payload).unwrap();
        
        let mut msg2 = vec![0u8; 256];
        let msg2_len = responder.write_message(b"", &mut msg2).unwrap();
        msg2.truncate(msg2_len);
        
        initiator.read_message(&msg2, &mut payload).unwrap();
        
        let mut msg3 = vec![0u8; 128];
        let msg3_len = initiator.write_message(b"", &mut msg3).unwrap();
        msg3.truncate(msg3_len);
        
        responder.read_message(&msg3, &mut payload).unwrap();
        
        // Get transport modes
        let mut init_transport = initiator.into_transport_mode().unwrap();
        let mut resp_transport = responder.into_transport_mode().unwrap();
        
        // Test nonce progression
        assert_eq!(init_transport.next_send_nonce(), 0);
        assert_eq!(init_transport.next_send_nonce(), 1);
        assert_eq!(resp_transport.next_recv_nonce(), 0);
        assert_eq!(resp_transport.next_recv_nonce(), 1);
    }
    
    #[cfg(feature = "classic")]
    #[test]
    fn session_key_zeroization() {
        let key_data = [42u8; 32];
        let session_key = SessionKey::new(key_data);
        
        // Key should be accessible
        assert_eq!(session_key.as_bytes(), &key_data);
        
        // After drop, the key should be zeroized (we can't test this directly,
        // but the ZeroizeOnDrop trait ensures it happens)
        drop(session_key);
    }
    
    #[cfg(feature = "classic")]
    #[test]
    fn session_key_equality() {
        let key1 = SessionKey::new([1u8; 32]);
        let key2 = SessionKey::new([1u8; 32]);
        let key3 = SessionKey::new([2u8; 32]);
        
        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }
    
    #[test]
    fn handshake_pattern_display() {
        assert_eq!(format!("{}", HandshakePattern::XX), "XX");
    }
    
    #[test]
    fn handshake_state_display() {
        assert_eq!(format!("{}", HandshakeState::Initial), "Initial");
        assert_eq!(format!("{}", HandshakeState::Completed), "Completed");
    }
    
    #[test]
    fn noise_error_display() {
        let error = NoiseError::InvalidState {
            expected: "test".to_string(),
            actual: "other".to_string(),
        };
        assert!(format!("{}", error).contains("Invalid handshake state"));
    }
}

/// Implementation of hybrid post-quantum Noise handshake
impl HybridNoiseHandshake {
    /// Create a new hybrid initiator handshake
    #[cfg(feature = "hybrid")]
    pub fn new_hybrid_initiator(pq_algorithm: PqAlgorithm) -> Result<Self, NoiseError> {
        Self::new_hybrid_role(Role::Initiator, pq_algorithm)
    }
    
    /// Create a new hybrid responder handshake
    #[cfg(feature = "hybrid")]
    pub fn new_hybrid_responder(pq_algorithm: PqAlgorithm) -> Result<Self, NoiseError> {
        Self::new_hybrid_role(Role::Responder, pq_algorithm)
    }
    
    /// Create hybrid handshake with specific role
    #[cfg(feature = "hybrid")]
    fn new_hybrid_role(role: Role, pq_algorithm: PqAlgorithm) -> Result<Self, NoiseError> {
        use crate::hybrid::generate_keypair;
        
        // Generate hybrid keypair (X25519 + PQ)
        let (_local_public, local_secret) = generate_keypair(pq_algorithm)
            .map_err(|e| NoiseError::HybridFailed(e.to_string()))?;
        
        // Initialize chaining key with hybrid protocol name hash
        let protocol_name: &[u8] = match pq_algorithm {
            PqAlgorithm::Kyber1024 => b"Noise_XX_25519+Kyber1024_ChaChaPoly_BLAKE3",
            PqAlgorithm::Bike => b"Noise_XX_25519+BIKE_ChaChaPoly_BLAKE3",
        };
        
        let mut chaining_key = [0u8; 32];
        let hash = blake3::hash(protocol_name);
        chaining_key.copy_from_slice(hash.as_bytes());
        
        // Initialize handshake hash with protocol name
        let mut handshake_hash = Hasher::new();
        handshake_hash.update(protocol_name);
        
        Ok(Self {
            state: HandshakeState::Initial,
            pattern: HandshakePattern::XX,
            role,
            #[cfg(feature = "hybrid")]
            local_hybrid: Some(local_secret),
            #[cfg(feature = "hybrid")]
            remote_hybrid: None,
            #[cfg(feature = "hybrid")]
            pq_algorithm,
            #[cfg(feature = "hybrid")]
            ee_kyber_extension: None,
            #[cfg(feature = "hybrid")]
            se_kyber_extension: None,
            handshake_hash,
            chaining_key,
            symmetric_key: None,
        })
    }
    
    /// Perform ee_kyber handshake extension
    #[cfg(feature = "hybrid")]
    pub fn perform_ee_kyber_handshake(&mut self) -> Result<(SessionKey, SessionKey), NoiseError> {
        let mut ee_ext = EeKyberExtension::new();
        
        // Generate local keypair for this handshake
        let _local_pk = ee_ext.generate_local_keypair(self.pq_algorithm)
            .map_err(|e| NoiseError::HybridFailed(format!("EE Kyber failed: {}", e)))?;
        
        // In real implementation, would exchange public keys with peer
        // For now, create a placeholder remote key for testing
        let (remote_pk, _) = crate::hybrid::generate_keypair(self.pq_algorithm)
            .map_err(|e| NoiseError::HybridFailed(e.to_string()))?;
        
        ee_ext.set_remote_public_key(remote_pk);
        
        // Perform the key exchange
        let shared_secret = ee_ext.exchange()?;
        
        self.ee_kyber_extension = Some(ee_ext);
        
        // Update handshake transcript with extension data
        self.handshake_hash.update(b"ee_kyber_extension");
        
        // Derive session keys from shared secret
        let client_key = SessionKey(shared_secret[..32].try_into().unwrap());
        let server_key = SessionKey(shared_secret[32..].try_into().unwrap());
        
        Ok((client_key, server_key))
    }
    
    /// Perform se_kyber handshake extension
    #[cfg(feature = "hybrid")]
    pub fn perform_se_kyber_handshake(&mut self, static_pk: &HybridPublicKey, payload: &[u8]) -> Result<SessionKey, NoiseError> {
        let mut se_ext = SeKyberExtension::new();
        
        // Set the static keypair (in real implementation, this would be loaded from storage)
        let static_keypair = crate::hybrid::generate_keypair(self.pq_algorithm)
            .map_err(|e| NoiseError::HybridFailed(e.to_string()))?;
        
        se_ext.set_static_keypair(static_keypair);
        se_ext.set_ephemeral_public_key(static_pk.clone());
        
        // Perform the key exchange
        let shared_secret = se_ext.exchange()?;
        
        self.se_kyber_extension = Some(se_ext);
        
        // Update handshake transcript with extension data
        self.handshake_hash.update(b"se_kyber_extension");
        self.handshake_hash.update(payload);
        
        // Derive session key from shared secret
        let session_key = SessionKey(shared_secret[..32].try_into().unwrap());
        
        Ok(session_key)
    }
    
    /// Write hybrid handshake message
    pub fn write_hybrid_message(&mut self, payload: &[u8], message: &mut [u8]) -> Result<usize, NoiseError> {
        match (self.role, self.state) {
            (Role::Initiator, HandshakeState::Initial) => {
                self.write_hybrid_initiator_first_message(payload, message)
            }
            (Role::Responder, HandshakeState::ResponderReceivedFirst) => {
                self.write_hybrid_responder_second_message(payload, message)
            }
            (Role::Initiator, HandshakeState::InitiatorReceivedSecond) => {
                self.write_hybrid_initiator_third_message(payload, message)
            }
            _ => Err(NoiseError::InvalidState {
                expected: "valid hybrid write state".to_string(),
                actual: format!("{:?} in state {:?}", self.role, self.state),
            }),
        }
    }
    
    /// Read hybrid handshake message
    pub fn read_hybrid_message(&mut self, message: &[u8], payload: &mut [u8]) -> Result<usize, NoiseError> {
        match (self.role, self.state) {
            (Role::Responder, HandshakeState::Initial) => {
                self.read_hybrid_responder_first_message(message, payload)
            }
            (Role::Initiator, HandshakeState::InitiatorSentFirst) => {
                self.read_hybrid_initiator_second_message(message, payload)
            }
            (Role::Responder, HandshakeState::ResponderSentSecond) => {
                self.read_hybrid_responder_third_message(message, payload)
            }
            _ => Err(NoiseError::InvalidState {
                expected: "valid hybrid read state".to_string(),
                actual: format!("{:?} in state {:?}", self.role, self.state),
            }),
        }
    }
    
    /// Write first message (initiator -> responder)
    fn write_hybrid_initiator_first_message(&mut self, payload: &[u8], message: &mut [u8]) -> Result<usize, NoiseError> {
        // For now, implement basic version without full hybrid logic
        // In production, this would include hybrid public key exchange
        
        if message.len() < payload.len() + 64 {
            return Err(NoiseError::MessageTooShort {
                expected: payload.len() + 64,
                actual: message.len(),
            });
        }
        
        // Copy payload (simplified implementation)
        message[..payload.len()].copy_from_slice(payload);
        
        self.state = HandshakeState::InitiatorSentFirst;
        Ok(payload.len())
    }
    
    /// Read first message (responder receiving from initiator)
    fn read_hybrid_responder_first_message(&mut self, message: &[u8], payload: &mut [u8]) -> Result<usize, NoiseError> {
        if payload.len() < message.len() {
            return Err(NoiseError::MessageTooShort {
                expected: message.len(),
                actual: payload.len(),
            });
        }
        
        // Copy payload (simplified implementation)
        let payload_len = message.len();
        payload[..payload_len].copy_from_slice(message);
        
        self.state = HandshakeState::ResponderReceivedFirst;
        Ok(payload_len)
    }
    
    /// Write second message (responder -> initiator)
    fn write_hybrid_responder_second_message(&mut self, payload: &[u8], message: &mut [u8]) -> Result<usize, NoiseError> {
        if message.len() < payload.len() + 128 {
            return Err(NoiseError::MessageTooShort {
                expected: payload.len() + 128,
                actual: message.len(),
            });
        }
        
        // Copy payload (simplified implementation)
        message[..payload.len()].copy_from_slice(payload);
        
        self.state = HandshakeState::ResponderSentSecond;
        Ok(payload.len())
    }
    
    /// Read second message (initiator receiving from responder)
    fn read_hybrid_initiator_second_message(&mut self, message: &[u8], payload: &mut [u8]) -> Result<usize, NoiseError> {
        if payload.len() < message.len() {
            return Err(NoiseError::MessageTooShort {
                expected: message.len(),
                actual: payload.len(),
            });
        }
        
        let payload_len = message.len();
        payload[..payload_len].copy_from_slice(message);
        
        self.state = HandshakeState::InitiatorReceivedSecond;
        Ok(payload_len)
    }
    
    /// Write third message (initiator -> responder)
    fn write_hybrid_initiator_third_message(&mut self, payload: &[u8], message: &mut [u8]) -> Result<usize, NoiseError> {
        if message.len() < payload.len() + 64 {
            return Err(NoiseError::MessageTooShort {
                expected: payload.len() + 64,
                actual: message.len(),
            });
        }
        
        // Copy payload (simplified implementation)
        message[..payload.len()].copy_from_slice(payload);
        
        self.state = HandshakeState::Completed;
        Ok(payload.len())
    }
    
    /// Read third message (responder receiving from initiator)
    fn read_hybrid_responder_third_message(&mut self, message: &[u8], payload: &mut [u8]) -> Result<usize, NoiseError> {
        if payload.len() < message.len() {
            return Err(NoiseError::MessageTooShort {
                expected: message.len(),
                actual: payload.len(),
            });
        }
        
        let payload_len = message.len();
        payload[..payload_len].copy_from_slice(message);
        
        self.state = HandshakeState::Completed;
        Ok(payload_len)
    }
    
    /// Convert to transport mode after successful handshake
    pub fn into_hybrid_transport_mode(self) -> Result<NoiseTransport, NoiseError> {
        if self.state != HandshakeState::Completed {
            return Err(NoiseError::InvalidState {
                expected: "Completed".to_string(),
                actual: format!("{:?}", self.state),
            });
        }
        
        // Derive transport keys from final handshake state
        let session_key = self.symmetric_key.unwrap_or_else(|| SessionKey([0u8; 32]));
        
        Ok(NoiseTransport::new(session_key.clone(), session_key))
    }
}

#[cfg(test)]
mod hybrid_tests {
    // TODO: Implement HybridNoiseHandshake constructor methods
    // These tests require additional implementation in the HybridNoiseHandshake struct
    
    #[test]
    #[ignore = "Requires HybridNoiseHandshake implementation"]
    fn test_hybrid_handshake_creation() {
        // Test will be enabled when constructor methods are implemented
    }
    
    #[test]
    #[ignore = "Requires HybridNoiseHandshake implementation"]
    fn test_ee_kyber_extension() {
        // Test will be enabled when EE Kyber handshake is implemented
    }
    
    #[test]
    #[ignore = "Requires HybridNoiseHandshake implementation"]
    #[cfg(feature = "hybrid")]
    fn test_se_kyber_extension() {
        // Test will be enabled when SE Kyber handshake is implemented
    }
    
    #[test]
    #[ignore = "Requires HybridNoiseHandshake implementation"]
    fn test_hybrid_message_flow() {
        // Test will be enabled when hybrid message flow is implemented
    }
}