//! Hybrid Post-Quantum Handshake Implementation for Nyx Protocol v1.0
//!
//! This module implements the hybrid cryptographic handshake combining:
//! - **ML-KEM-768**: NIST-standardized post-quantum KEM (AES-192 equivalent security)
//! - **X25519**: Classical elliptic curve Diffie-Hellman for immediate security
//!
//! ## Security Properties
//!
//! - **Quantum Resistance**: ML-KEM provides security against quantum computer attacks
//! - **Classical Security**: X25519 provides fallback security against classical attacks  
//! - **Forward Secrecy**: Both ML-KEM and X25519 keys are ephemeral per session
//! - **Hybrid Security**: Combined approach provides security even if one algorithm fails
//! - **NIST Compliance**: Follows NIST standards for post-quantum cryptography
//!
//! ## Protocol Flow
//!
//! 1. **Client**: Generate ML-KEM-768 + X25519 key pairs
//! 2. **Client→Server**: Send combined public keys
//! 3. **Server**: Generate own key pairs, perform encapsulation/key exchange
//! 4. **Server→Client**: Send encapsulated secrets
//! 5. **Both**: Derive final shared secret using secure KDF
//!
//! ## Implementation Notes
//!
//! - Uses `ml-kem` crate for pure Rust ML-KEM implementation (no C dependencies)
//! - Uses `x25519-dalek` crate for X25519 implementation  
//! - HKDF-SHA256 for secure key derivation
//! - Constant-time operations to prevent side-channel attacks
//! - Comprehensive input validation and error handling

#![forbid(unsafe_code)]

use crate::{Error, Result};
use hkdf::Hkdf;
use ml_kem::kem::{Decapsulate, Encapsulate};
use ml_kem::{Ciphertext, EncodedSizeUser, KemCore};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use tracing::{debug, info, warn};
use x25519_dalek::{EphemeralSecret, PublicKey as X25519PublicKey, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop};

// ML-KEM-768パラメータのサイズ定数
// 実際のサイズは実行時に確定するため、適切な値を使用

/// Size of ML-KEM-768 public key in bytes (実際は1184バイト)
pub const KYBER_PUBLIC_KEY_SIZE: usize = 1184;

/// Size of ML-KEM-768 secret key in bytes (実際は2400バイト)
pub const KYBER_SECRET_KEY_SIZE: usize = 2400;

/// Size of ML-KEM-768 ciphertext in bytes (実際は1088バイト)
pub const KYBER_CIPHERTEXT_SIZE: usize = 1088;

/// Size of ML-KEM-768 shared secret in bytes (32バイト)
pub const KYBER_SHARED_SECRET_SIZE: usize = 32;

/// Size of X25519 public key in bytes (32 bytes)
pub const X25519_PUBLIC_KEY_SIZE: usize = 32;

/// Size of X25519 private key in bytes (32 bytes)
pub const X25519_SECRET_KEY_SIZE: usize = 32;

/// Size of combined hybrid public key (Kyber + X25519)
pub const HYBRID_PUBLIC_KEY_SIZE: usize = KYBER_PUBLIC_KEY_SIZE + X25519_PUBLIC_KEY_SIZE;

/// Size of final derived shared secret (32 bytes)
pub const SHARED_SECRET_SIZE: usize = 32;

/// Maximum additional data size for KDF context (prevents DoS)
pub const MAX_ADDITIONAL_DATA_SIZE: usize = 1024;

/// ML-KEM-768 public key wrapper with secure handling
#[derive(Clone, PartialEq, Eq)]
pub struct KyberPublicKey {
    /// Raw ML-KEM-768 public key bytes
    bytes: Box<[u8; KYBER_PUBLIC_KEY_SIZE]>,
}

impl KyberPublicKey {
    /// Create a new ML-KEM public key from bytes
    pub fn from_bytes(bytes: [u8; KYBER_PUBLIC_KEY_SIZE]) -> Self {
        Self {
            bytes: Box::new(bytes),
        }
    }

    /// Get the raw bytes of the public key
    pub fn as_bytes(&self) -> &[u8; KYBER_PUBLIC_KEY_SIZE] {
        &self.bytes
    }

    /// Convert to raw bytes array
    pub fn to_bytes(&self) -> [u8; KYBER_PUBLIC_KEY_SIZE] {
        *self.bytes
    }
}

impl fmt::Debug for KyberPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KyberPublicKey")
            .field("size", &KYBER_PUBLIC_KEY_SIZE)
            .field("hash", &format!("{:08x}", self.hash()))
            .finish()
    }
}

impl KyberPublicKey {
    /// Compute a hash of the public key for debugging/logging
    fn hash(&self) -> u32 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.bytes.as_slice().hash(&mut hasher);
        hasher.finish() as u32
    }
}

/// ML-KEM-768 secret key with secure memory handling
#[derive(ZeroizeOnDrop)]
pub struct KyberSecretKey {
    /// Raw ML-KEM-768 secret key bytes
    bytes: Box<[u8; KYBER_SECRET_KEY_SIZE]>,
}

impl KyberSecretKey {
    /// Create a new ML-KEM secret key from bytes
    pub fn from_bytes(bytes: [u8; KYBER_SECRET_KEY_SIZE]) -> Self {
        Self {
            bytes: Box::new(bytes),
        }
    }

    /// Get the raw bytes of the secret key
    pub fn as_bytes(&self) -> &[u8; KYBER_SECRET_KEY_SIZE] {
        &self.bytes
    }
}

impl fmt::Debug for KyberSecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KyberSecretKey")
            .field("size", &KYBER_SECRET_KEY_SIZE)
            .field("status", &"[REDACTED]")
            .finish()
    }
}

/// X25519 public key wrapper
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct X25519PublicKeyWrapper {
    /// Raw X25519 public key bytes
    bytes: [u8; X25519_PUBLIC_KEY_SIZE],
}

impl X25519PublicKeyWrapper {
    /// Create from X25519 public key
    pub fn from_x25519(key: X25519PublicKey) -> Self {
        Self {
            bytes: key.to_bytes(),
        }
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; X25519_PUBLIC_KEY_SIZE]) -> Self {
        Self { bytes }
    }

    /// Get the raw bytes
    pub fn as_bytes(&self) -> &[u8; X25519_PUBLIC_KEY_SIZE] {
        &self.bytes
    }

    /// Convert to raw bytes
    pub fn to_bytes(&self) -> [u8; X25519_PUBLIC_KEY_SIZE] {
        self.bytes
    }

    /// Convert to X25519 public key
    pub fn to_x25519(&self) -> X25519PublicKey {
        X25519PublicKey::from(self.bytes)
    }
}

impl fmt::Debug for X25519PublicKeyWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("X25519PublicKey")
            .field("size", &X25519_PUBLIC_KEY_SIZE)
            .field(
                "prefix",
                &format!("{:02x}{:02x}...", self.bytes[0], self.bytes[1]),
            )
            .finish()
    }
}

/// Combined hybrid public key (Kyber-768 + X25519)
#[derive(Clone, PartialEq, Eq)]
pub struct HybridPublicKey {
    /// Kyber-768 public key component
    pub kyber: KyberPublicKey,
    /// X25519 public key component  
    pub x25519: X25519PublicKeyWrapper,
}

impl HybridPublicKey {
    /// Create a new hybrid public key
    pub fn new(kyber: KyberPublicKey, x25519: X25519PublicKeyWrapper) -> Self {
        Self { kyber, x25519 }
    }

    /// Serialize to wire format (Kyber || X25519)
    pub fn to_wire_format(&self) -> [u8; HYBRID_PUBLIC_KEY_SIZE] {
        let mut result = [0u8; HYBRID_PUBLIC_KEY_SIZE];

        // First part: Kyber public key
        result[..KYBER_PUBLIC_KEY_SIZE].copy_from_slice(self.kyber.as_bytes());

        // Second part: X25519 public key
        result[KYBER_PUBLIC_KEY_SIZE..].copy_from_slice(self.x25519.as_bytes());

        result
    }

    /// Deserialize from wire format
    pub fn from_wire_format(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != HYBRID_PUBLIC_KEY_SIZE {
            return Err(Error::Protocol(format!(
                "Invalid hybrid public key size: expected {}, got {}",
                HYBRID_PUBLIC_KEY_SIZE,
                bytes.len()
            )));
        }

        // Extract Kyber public key
        let mut kyber_bytes = [0u8; KYBER_PUBLIC_KEY_SIZE];
        kyber_bytes.copy_from_slice(&bytes[..KYBER_PUBLIC_KEY_SIZE]);
        let kyber = KyberPublicKey::from_bytes(kyber_bytes);

        // Extract X25519 public key
        let mut x25519_bytes = [0u8; X25519_PUBLIC_KEY_SIZE];
        x25519_bytes.copy_from_slice(&bytes[KYBER_PUBLIC_KEY_SIZE..]);
        let x25519 = X25519PublicKeyWrapper::from_bytes(x25519_bytes);

        Ok(Self::new(kyber, x25519))
    }

    /// Get the total size of the hybrid public key
    pub fn size(&self) -> usize {
        HYBRID_PUBLIC_KEY_SIZE
    }
}

impl fmt::Debug for HybridPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HybridPublicKey")
            .field("kyber", &self.kyber)
            .field("x25519", &self.x25519)
            .field("total_size", &HYBRID_PUBLIC_KEY_SIZE)
            .finish()
    }
}

/// Hybrid key pair (Kyber-768 + X25519) with secure key management
pub struct HybridKeyPair {
    /// ML-KEM-768 encapsulation key (public)
    kyber_encaps_key: ml_kem::kem::EncapsulationKey<ml_kem::MlKem768Params>,
    /// ML-KEM-768 decapsulation key (secret)
    kyber_decaps_key: ml_kem::kem::DecapsulationKey<ml_kem::MlKem768Params>,
    /// X25519 secret key
    x25519_secret: StaticSecret,
    /// X25519 public key
    x25519_public: X25519PublicKeyWrapper,
}

impl HybridKeyPair {
    /// Generate a new hybrid key pair using secure randomness
    pub fn generate() -> Result<Self> {
        info!("Generating new hybrid post-quantum key pair");

        // Generate ML-KEM-768 key pair
        let mut rng = OsRng;
        let (kyber_decaps_key, kyber_encaps_key) = ml_kem::MlKem768::generate(&mut rng);

        // Generate X25519 key pair
        let x25519_secret = StaticSecret::random_from_rng(rng);
        let x25519_public_raw = X25519PublicKey::from(&x25519_secret);
        let x25519_public = X25519PublicKeyWrapper::from_x25519(x25519_public_raw);

        debug!(
            kyber_size = KYBER_PUBLIC_KEY_SIZE,
            x25519_size = X25519_PUBLIC_KEY_SIZE,
            total_size = HYBRID_PUBLIC_KEY_SIZE,
            "Generated hybrid key pair successfully"
        );

        Ok(Self {
            kyber_encaps_key,
            kyber_decaps_key,
            x25519_secret,
            x25519_public,
        })
    }

    /// Get the public key component
    pub fn public_key(&self) -> HybridPublicKey {
        let kyber_public = KyberPublicKey::from_bytes(
            self.kyber_encaps_key
                .as_bytes()
                .as_slice()
                .try_into()
                .expect("Invalid encaps key size"),
        );
        HybridPublicKey::new(kyber_public, self.x25519_public)
    }

    /// Perform key encapsulation (server side)
    /// Returns (ciphertext, shared_secret)
    pub fn encapsulate(
        client_public: &HybridPublicKey,
    ) -> Result<(HybridCiphertext, SharedSecret)> {
        info!("Performing hybrid key encapsulation");

        // ML-KEM encapsulation
        let mut rng = OsRng;

        // Create encapsulation key from public key bytes
        let kyber_public_bytes = client_public.kyber.as_bytes();
        let client_encaps_key = ml_kem::kem::EncapsulationKey::<ml_kem::MlKem768Params>::from_bytes(
            kyber_public_bytes.into(),
        );

        let (kyber_ciphertext, kyber_shared_secret) = client_encaps_key
            .encapsulate(&mut rng)
            .map_err(|_| Error::Protocol("ML-KEM encapsulation failed".to_string()))?;

        // X25519 key exchange (server generates ephemeral key)
        let server_x25519_secret = EphemeralSecret::random_from_rng(rng);
        let server_x25519_public = X25519PublicKey::from(&server_x25519_secret);

        let x25519_shared_secret =
            server_x25519_secret.diffie_hellman(&client_public.x25519.to_x25519());

        // Combine the shared secrets using KDF
        let combined_shared_secret = Self::derive_shared_secret(
            kyber_shared_secret.as_slice(),
            x25519_shared_secret.as_bytes(),
            &client_public.to_wire_format(),
            &server_x25519_public.to_bytes(),
        )?;

        let ciphertext = HybridCiphertext {
            kyber_ciphertext: kyber_ciphertext.as_slice().try_into().map_err(|_| {
                Error::Protocol(format!(
                    "Invalid ML-KEM ciphertext size: expected {}, got {}",
                    KYBER_CIPHERTEXT_SIZE,
                    kyber_ciphertext.as_slice().len()
                ))
            })?,
            x25519_public: X25519PublicKeyWrapper::from_x25519(server_x25519_public),
        };

        debug!(
            kyber_ciphertext_size = KYBER_CIPHERTEXT_SIZE,
            x25519_public_size = X25519_PUBLIC_KEY_SIZE,
            shared_secret_size = SHARED_SECRET_SIZE,
            "Key encapsulation completed successfully"
        );

        Ok((ciphertext, combined_shared_secret))
    }

    /// Perform key decapsulation (client side)
    pub fn decapsulate(&self, ciphertext: &HybridCiphertext) -> Result<SharedSecret> {
        info!("Performing hybrid key decapsulation");

        // ML-KEM decapsulation
        let kyber_ciphertext_bytes =
            Ciphertext::<ml_kem::MlKem768>::try_from(&ciphertext.kyber_ciphertext[..])
                .map_err(|_| Error::Protocol("Invalid ML-KEM ciphertext size".to_string()))?;

        let kyber_shared_secret = self
            .kyber_decaps_key
            .decapsulate(&kyber_ciphertext_bytes)
            .map_err(|_| Error::Protocol("ML-KEM decapsulation failed".to_string()))?;

        // X25519 key exchange
        let x25519_shared_secret = self
            .x25519_secret
            .diffie_hellman(&ciphertext.x25519_public.to_x25519());

        // Combine the shared secrets using KDF
        let client_public = self.public_key();
        let combined_shared_secret = Self::derive_shared_secret(
            kyber_shared_secret.as_slice(),
            x25519_shared_secret.as_bytes(),
            &client_public.to_wire_format(),
            &ciphertext.x25519_public.to_bytes(),
        )?;

        debug!(
            shared_secret_size = SHARED_SECRET_SIZE,
            "Key decapsulation completed successfully"
        );

        Ok(combined_shared_secret)
    }

    /// Derive the final shared secret using HKDF
    fn derive_shared_secret(
        kyber_secret: &[u8],
        x25519_secret: &[u8; 32],
        client_public: &[u8],
        server_public: &[u8],
    ) -> Result<SharedSecret> {
        // Input key material: ML-KEM secret || X25519 secret
        let mut ikm = Vec::with_capacity(kyber_secret.len() + 32);
        ikm.extend_from_slice(kyber_secret);
        ikm.extend_from_slice(x25519_secret);

        // Salt: Hash of both public keys for domain separation
        let mut salt_input = Vec::with_capacity(client_public.len() + server_public.len());
        salt_input.extend_from_slice(client_public);
        salt_input.extend_from_slice(server_public);

        let salt = Sha256::digest(&salt_input);

        // Info: Protocol identifier and version
        let info = b"Nyx-v1.0-Hybrid-PQ-Handshake";

        // HKDF-SHA256 key derivation
        let hkdf = Hkdf::<Sha256>::new(Some(&salt), &ikm);
        let mut okm = [0u8; SHARED_SECRET_SIZE];

        hkdf.expand(info, &mut okm)
            .map_err(|e| Error::Protocol(format!("HKDF expansion failed: {e}")))?;

        // Zeroize intermediate values
        ikm.zeroize();
        salt_input.zeroize();

        debug!(
            ikm_size = kyber_secret.len() + 32,
            salt_size = salt.len(),
            info_size = info.len(),
            output_size = SHARED_SECRET_SIZE,
            "Shared secret derivation completed"
        );

        Ok(SharedSecret::new(okm))
    }
}

impl fmt::Debug for HybridKeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HybridKeyPair")
            .field("kyber_encaps_key", &"[ML-KEM-768 public key]")
            .field("x25519_public", &self.x25519_public)
            .field("secrets", &"[REDACTED]")
            .finish()
    }
}

/// Hybrid ciphertext containing encapsulated secrets
#[derive(Clone)]
pub struct HybridCiphertext {
    /// Kyber-768 ciphertext
    pub kyber_ciphertext: [u8; KYBER_CIPHERTEXT_SIZE],
    /// Server's X25519 public key
    pub x25519_public: X25519PublicKeyWrapper,
}

impl HybridCiphertext {
    /// Serialize to wire format
    pub fn to_wire_format(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(KYBER_CIPHERTEXT_SIZE + X25519_PUBLIC_KEY_SIZE);
        result.extend_from_slice(&self.kyber_ciphertext);
        result.extend_from_slice(self.x25519_public.as_bytes());
        result
    }

    /// Deserialize from wire format
    pub fn from_wire_format(bytes: &[u8]) -> Result<Self> {
        let expected_size = KYBER_CIPHERTEXT_SIZE + X25519_PUBLIC_KEY_SIZE;
        if bytes.len() != expected_size {
            return Err(Error::Protocol(format!(
                "Invalid hybrid ciphertext size: expected {}, got {}",
                expected_size,
                bytes.len()
            )));
        }

        let mut kyber_ciphertext = [0u8; KYBER_CIPHERTEXT_SIZE];
        kyber_ciphertext.copy_from_slice(&bytes[..KYBER_CIPHERTEXT_SIZE]);

        let mut x25519_bytes = [0u8; X25519_PUBLIC_KEY_SIZE];
        x25519_bytes.copy_from_slice(&bytes[KYBER_CIPHERTEXT_SIZE..]);
        let x25519_public = X25519PublicKeyWrapper::from_bytes(x25519_bytes);

        Ok(Self {
            kyber_ciphertext,
            x25519_public,
        })
    }

    /// Get the total size of the ciphertext
    pub fn size(&self) -> usize {
        KYBER_CIPHERTEXT_SIZE + X25519_PUBLIC_KEY_SIZE
    }
}

impl fmt::Debug for HybridCiphertext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HybridCiphertext")
            .field("kyber_ciphertext_size", &KYBER_CIPHERTEXT_SIZE)
            .field("x25519_public", &self.x25519_public)
            .field("total_size", &self.size())
            .finish()
    }
}

/// Final shared secret with secure memory handling
#[derive(ZeroizeOnDrop)]
pub struct SharedSecret {
    /// The derived shared secret bytes
    bytes: [u8; SHARED_SECRET_SIZE],
}

impl SharedSecret {
    /// Create a new shared secret
    pub fn new(bytes: [u8; SHARED_SECRET_SIZE]) -> Self {
        Self { bytes }
    }

    /// Get the raw bytes of the shared secret
    pub fn as_bytes(&self) -> &[u8; SHARED_SECRET_SIZE] {
        &self.bytes
    }

    /// Convert to raw bytes (consumes the secret)
    pub fn into_bytes(self) -> [u8; SHARED_SECRET_SIZE] {
        self.bytes
    }
}

impl fmt::Debug for SharedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SharedSecret")
            .field("size", &SHARED_SECRET_SIZE)
            .field("status", &"[REDACTED]")
            .finish()
    }
}

/// Complete hybrid handshake implementation
pub struct HybridHandshake;

impl HybridHandshake {
    /// Client-side: Generate key pair and create initial message
    pub fn client_init() -> Result<(HybridKeyPair, HybridPublicKey)> {
        info!("Initializing client-side hybrid handshake");

        let key_pair = HybridKeyPair::generate()?;
        let public_key = key_pair.public_key();

        debug!(
            public_key_size = public_key.size(),
            "Client handshake initialization complete"
        );

        Ok((key_pair, public_key))
    }

    /// Server-side: Process client message and generate response
    pub fn server_respond(
        client_public: &HybridPublicKey,
    ) -> Result<(HybridCiphertext, SharedSecret)> {
        info!("Processing client handshake and generating server response");

        // Validate client public key
        Self::validate_public_key(client_public)?;

        // Perform encapsulation
        let (ciphertext, shared_secret) = HybridKeyPair::encapsulate(client_public)?;

        debug!(
            ciphertext_size = ciphertext.size(),
            "Server handshake response generated"
        );

        Ok((ciphertext, shared_secret))
    }

    /// Client-side: Process server response and derive shared secret
    pub fn client_finalize(
        key_pair: &HybridKeyPair,
        server_ciphertext: &HybridCiphertext,
    ) -> Result<SharedSecret> {
        info!("Finalizing client-side handshake");

        // Validate server ciphertext
        Self::validate_ciphertext(server_ciphertext)?;

        // Perform decapsulation
        let shared_secret = key_pair.decapsulate(server_ciphertext)?;

        debug!("Client handshake finalization complete");

        Ok(shared_secret)
    }

    /// Validate a hybrid public key for security
    fn validate_public_key(public_key: &HybridPublicKey) -> Result<()> {
        // Basic size validation is handled by type system

        // Additional validation: Check for obviously invalid keys
        let kyber_bytes = public_key.kyber.as_bytes();
        let x25519_bytes = public_key.x25519.as_bytes();

        // Check for all-zero keys (invalid)
        if kyber_bytes.iter().all(|&b| b == 0) {
            warn!("SECURITY: Detected all-zero Kyber public key");
            return Err(Error::Protocol(
                "Invalid Kyber public key: all zeros".to_string(),
            ));
        }

        if x25519_bytes.iter().all(|&b| b == 0) {
            warn!("SECURITY: Detected all-zero X25519 public key");
            return Err(Error::Protocol(
                "Invalid X25519 public key: all zeros".to_string(),
            ));
        }

        // Check for all-ones keys (also suspicious)
        if kyber_bytes.iter().all(|&b| b == 0xff) {
            warn!("SECURITY: Detected all-ones Kyber public key");
            return Err(Error::Protocol(
                "Invalid Kyber public key: all ones".to_string(),
            ));
        }

        if x25519_bytes.iter().all(|&b| b == 0xff) {
            warn!("SECURITY: Detected all-ones X25519 public key");
            return Err(Error::Protocol(
                "Invalid X25519 public key: all ones".to_string(),
            ));
        }

        debug!("Public key validation passed");
        Ok(())
    }

    /// Validate a hybrid ciphertext for security  
    fn validate_ciphertext(ciphertext: &HybridCiphertext) -> Result<()> {
        // Basic size validation is handled by type system

        // Check X25519 public key in ciphertext
        let x25519_bytes = ciphertext.x25519_public.as_bytes();

        if x25519_bytes.iter().all(|&b| b == 0) {
            warn!("SECURITY: Detected all-zero X25519 public key in ciphertext");
            return Err(Error::Protocol(
                "Invalid X25519 public key in ciphertext: all zeros".to_string(),
            ));
        }

        if x25519_bytes.iter().all(|&b| b == 0xff) {
            warn!("SECURITY: Detected all-ones X25519 public key in ciphertext");
            return Err(Error::Protocol(
                "Invalid X25519 public key in ciphertext: all ones".to_string(),
            ));
        }

        debug!("Ciphertext validation passed");
        Ok(())
    }

    /// Get information about the hybrid cryptographic parameters
    pub fn get_parameters() -> HybridParameters {
        HybridParameters {
            kyber_variant: "ML-KEM-768".to_string(),
            kyber_security_level: "AES-192 equivalent".to_string(),
            x25519_security_level: "~128-bit classical".to_string(),
            public_key_size: HYBRID_PUBLIC_KEY_SIZE,
            ciphertext_size: KYBER_CIPHERTEXT_SIZE + X25519_PUBLIC_KEY_SIZE,
            shared_secret_size: SHARED_SECRET_SIZE,
            quantum_safe: true,
            forward_secure: true,
        }
    }
}

/// Information about hybrid cryptographic parameters
#[derive(Debug, Clone)]
pub struct HybridParameters {
    pub kyber_variant: String,
    pub kyber_security_level: String,
    pub x25519_security_level: String,
    pub public_key_size: usize,
    pub ciphertext_size: usize,
    pub shared_secret_size: usize,
    pub quantum_safe: bool,
    pub forward_secure: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_generation() -> Result<()> {
        let key_pair = HybridKeyPair::generate()?;
        let public_key = key_pair.public_key();

        assert_eq!(public_key.size(), HYBRID_PUBLIC_KEY_SIZE);
        assert_eq!(public_key.kyber.as_bytes().len(), KYBER_PUBLIC_KEY_SIZE);
        assert_eq!(public_key.x25519.as_bytes().len(), X25519_PUBLIC_KEY_SIZE);

        Ok(())
    }

    #[test]
    fn test_public_key_serialization() -> Result<()> {
        let key_pair = HybridKeyPair::generate()?;
        let public_key = key_pair.public_key();

        let wire_format = public_key.to_wire_format();
        assert_eq!(wire_format.len(), HYBRID_PUBLIC_KEY_SIZE);

        let reconstructed = HybridPublicKey::from_wire_format(&wire_format)?;
        assert_eq!(public_key, reconstructed);

        Ok(())
    }

    #[test]
    fn test_complete_handshake() -> Result<()> {
        // Client side
        let (client_key_pair, client_public) = HybridHandshake::client_init()?;

        // Server side
        let (server_ciphertext, server_secret) = HybridHandshake::server_respond(&client_public)?;

        // Client side (finalize)
        let client_secret = HybridHandshake::client_finalize(&client_key_pair, &server_ciphertext)?;

        // Verify both sides derived the same secret
        assert_eq!(server_secret.as_bytes(), client_secret.as_bytes());

        Ok(())
    }

    #[test]
    fn test_ciphertext_serialization() -> Result<()> {
        let (client_key_pair, client_public) = HybridHandshake::client_init()?;
        let (ciphertext, _) = HybridHandshake::server_respond(&client_public)?;

        let wire_format = ciphertext.to_wire_format();
        let reconstructed = HybridCiphertext::from_wire_format(&wire_format)?;

        // Should be able to decapsulate with reconstructed ciphertext
        let secret1 = client_key_pair.decapsulate(&ciphertext)?;
        let secret2 = client_key_pair.decapsulate(&reconstructed)?;

        assert_eq!(secret1.as_bytes(), secret2.as_bytes());

        Ok(())
    }

    #[test]
    fn test_invalid_public_key_validation() {
        // Test all-zero Kyber key
        let zero_kyber = KyberPublicKey::from_bytes([0u8; KYBER_PUBLIC_KEY_SIZE]);
        let valid_x25519 = X25519PublicKeyWrapper::from_bytes([1u8; X25519_PUBLIC_KEY_SIZE]);
        let invalid_key = HybridPublicKey::new(zero_kyber, valid_x25519);

        assert!(HybridHandshake::validate_public_key(&invalid_key).is_err());

        // Test all-zero X25519 key
        let valid_kyber = KyberPublicKey::from_bytes([1u8; KYBER_PUBLIC_KEY_SIZE]);
        let zero_x25519 = X25519PublicKeyWrapper::from_bytes([0u8; X25519_PUBLIC_KEY_SIZE]);
        let invalid_key = HybridPublicKey::new(valid_kyber, zero_x25519);

        assert!(HybridHandshake::validate_public_key(&invalid_key).is_err());
    }

    #[test]
    fn test_parameters_info() {
        let params = HybridHandshake::get_parameters();

        assert_eq!(params.kyber_variant, "ML-KEM-768");
        assert_eq!(params.public_key_size, HYBRID_PUBLIC_KEY_SIZE);
        assert!(params.quantum_safe);
        assert!(params.forward_secure);
    }
}
