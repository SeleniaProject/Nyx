//! Nyx cryptography primitive_s and protocol_s (unsafe-forbid, WASM-friendly).
//! - AEAD: ChaCha20-Poly1305 wrapper with zeroizing key_s
//! - KDF: HKDF-SHA256 helper_s, RFC8439 nonce derivation
//! - HPKE (feature=hpke): X25519/HKDF-SHA256/AES-GCM-128
//! - Session: single-direction AEAD session with sequence/limit_s, rekey (record/byte_s), direction-id nonce separation
//! - Noise demo/guard_s: size cap_s, IK with tagged transcript AAD and optional 0-RTT
//!   - Backward-compatible wire header: 'N','X', ver, kind_flag_s
//!   - kind_flag_s: type (msg1/msg2) + flag_s (0-RTT, role bit_s)
//!   - Anti-downgrade: legacy (no header) must not carry 0-RTT; responder enforce_s
#![forbid(unsafe_code)]

pub mod aead;
pub mod hpke;
pub mod hybrid;
pub mod kdf;
pub mod keystore;
#[cfg(feature = "classic")]
pub mod noise;
pub mod pcr;
pub mod session;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error {
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Cryptographic operation failed: {0}")]
    Crypto(String),
    #[error("Invalid key: {0}")]
    InvalidKey(String),
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("Post-quantum operation failed: {0}")]
    PostQuantumError(String),
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),
}

pub type Result<T> = core::result::Result<T, Error>;

// Feature-gated ML-KEM wrapper (secure NIST-standardized post-quantum cryptography)
#[cfg(feature = "kyber")]
pub mod kyber;
