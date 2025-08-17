//! Hybrid (classic + PQ) handshake scaffolding.
//! This module prepares types and interfaces to implement a hybrid
//! Noise_Nyx pattern mixing X25519 and Kyber KEM. The full implementation
//! will:
//! - Perform parallel DH/KEM (es + ss with X25519, plus encapsulation with Kyber)
//! - Mix both secrets into the symmetric state (ck/h) with domain-separated labels
//! - Support 0-RTT early data under anti-replay constraints
//! - Provide re-handshake paths to switch to PQ-only when policy requests
//!
//! NOTE: The full wire format and anti-downgrade measures will be added next.

#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy)]
pub enum HybridKemKind {
	#[cfg(feature = "kyber")]
	Kyber,
}

#[derive(Debug, Clone)]
pub struct HybridConfig {
	pub kem: Option<HybridKemKind>,
	pub allow_0rtt: bool,
}

impl Default for HybridConfig {
	fn default() -> Self { Self { kem: None, allow_0rtt: true } }
}

/// Placeholder API that will be wired to `noise` once hybrid KEM is enabled.
pub struct HybridHandshake;

impl HybridHandshake {
	pub fn new(_cfg: HybridConfig) -> Self { Self }

	/// Returns whether hybrid KEM is effectively enabled (feature + config).
	pub fn is_enabled(&self) -> bool {
		#[cfg(feature = "kyber")]
		{ return true; }
		#[allow(unreachable_code)]
		false
	}
}
