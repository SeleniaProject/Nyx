
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
	#[error("{0}")]
	Protocol(String),
}

pub type Result<T> = core::result::Result<T, Error>;

// Feature-gated Kyber demo: stub before swapping in a real KEM implementation.
#[cfg(feature = "kyber")]
pub mod kyber_stub {
	/// Pseudo KEM session: derive same key from same seed, check only consistency
	pub fn kem_session_key_matches() -> bool {
		let seed = b"nyx-kyber-stub-seed-v1";
		let k1: [u8; 32] = *blake3::hash(seed).as_bytes();
		let k2: [u8; 32] = *blake3::hash(seed).as_bytes();
		k1 == k2
	}
}

