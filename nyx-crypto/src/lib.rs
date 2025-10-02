//!
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
#![warn(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

#![cfg_attr(feature = "strict-docs", warn(missing_docs))]
#![allow(missing_docs)]
#![allow(clippy::doc_lazy_continuation)]

//! Cryptographic primitives and protocol helpers for Nyx.
//!
//! This crate opts out of missing_docs warnings by default to reduce noise during development.
//! Enable the `strict-docs` feature to enforce documentation.

/// Authenticated encryption (ChaCha20-Poly1305) utilities.
pub mod aead;
/// Hybrid public-key encryption (HPKE) helpers.
pub mod hpke;
/// Hybrid KEM scaffolding and telemetry.
pub mod hybrid;
#[cfg(feature = "hybrid-handshake")]
pub mod hybrid_handshake;
/// Key derivation functions (HKDF-SHA256 etc.).
pub mod kdf;
/// Simple keystore helpers.
pub mod keystore;
#[cfg(feature = "classic")]
/// Noise-based demo handshake and utilities.
pub mod noise;
/// Platform configuration register helpers.
pub mod pcr;
/// AEAD session helpers and wrappers.
pub mod session;

/// Error type for cryptographic and protocol operations.
#[derive(thiserror::Error, Debug, PartialEq)]
pub enum Error {
    #[error("Protocol error: {0}")]
    /// Protocol layer error
    Protocol(String),
    #[error("Cryptographic operation failed: {0}")]
    /// Cryptographic operation failed
    Crypto(String),
    #[error("Invalid key: {0}")]
    /// Invalid key material or format
    InvalidKey(String),
    #[error("Authentication failed: {0}")]
    /// Authentication failure
    AuthenticationFailed(String),
    #[error("Post-quantum operation failed: {0}")]
    /// Post-quantum operation failed
    PostQuantumError(String),
    #[error("Feature not implemented: {0}")]
    /// Unimplemented feature
    NotImplemented(String),
}

/// Convenient Result alias for this crate.
pub type Result<T> = core::result::Result<T, Error>;

// Feature-gated ML-KEM wrapper (secure NIST-standardized post-quantum cryptography)
#[cfg(feature = "kyber")]
pub mod kyber;

// BIKE KEM placeholder module (feature-gated, not yet implemented)
#[cfg(feature = "bike")]
pub mod bike;

// Hybrid post-quantum handshake (Kyber-768 + X25519)
#[cfg(feature = "hybrid-handshake")]
pub use hybrid_handshake::{
    HybridCiphertext, HybridHandshake, HybridKeyPair, HybridParameters, HybridPublicKey,
    KyberPublicKey, KyberSecretKey, SharedSecret, X25519PublicKeyWrapper, HYBRID_PUBLIC_KEY_SIZE,
    KYBER_CIPHERTEXT_SIZE, KYBER_PUBLIC_KEY_SIZE, KYBER_SECRET_KEY_SIZE, KYBER_SHARED_SECRET_SIZE,
    MAX_ADDITIONAL_DATA_SIZE, SHARED_SECRET_SIZE, X25519_PUBLIC_KEY_SIZE, X25519_SECRET_KEY_SIZE,
};
