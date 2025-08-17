
//! Nyx cryptography primitives and protocols (unsafe-forbid, WASM-friendly).
//! - AEAD: ChaCha20-Poly1305 wrapper with zeroizing keys
//! - KDF: HKDF-SHA256 helpers, RFC8439 nonce derivation
//! - HPKE (feature=hpke): X25519/HKDF-SHA256/AES-GCM-128
//! - Session: single-direction AEAD session with sequence/limits, rekey (record/bytes), direction-id nonce separation
//! - Noise demo/guards: size caps, IK with tagged transcript AAD and optional 0-RTT
//!   - Backward-compatible wire header: 'N','X', ver, kind_flags
//!   - kind_flags: type (msg1/msg2) + flags (0-RTT, role bits)
//!   - Anti-downgrade: legacy (no header) must not carry 0-RTT; responder enforces
#![forbid(unsafe_code)]

#[cfg(feature = "classic")]
pub mod noise;
pub mod hpke;
pub mod aead;
pub mod kdf;
pub mod session;
pub mod hybrid;
pub mod keystore;
pub mod pcr;

#[derive(thiserror::Error, Debug)]
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
}

pub type Result<T> = core::result::Result<T, Error>;

// Feature-gated Kyber KEM wrapper (pure Rust implementation via pqc_kyber)
#[cfg(feature = "kyber")]
pub mod kyber;

