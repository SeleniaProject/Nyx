//! Minimal helpers to exercise rekey flows at the stream layer.
//! This does not perform HPKE itself; it relies on nyx-crypto sessions.
#![forbid(unsafe_code)]

use nyx_crypto::aead::{AeadKey, AeadSuite};
use nyx_crypto::session::AeadSession;

/// Small facade to create paired TX/RX sessions and tick counters to hit rekey.
pub struct RekeyHarness {
	pub tx: AeadSession,
	pub rx: AeadSession,
}

impl RekeyHarness {
	/// Build a pair with the same initial key/nonce and a record-based rekey interval.
	pub fn new_with_record_threshold(threshold: u64) -> Self {
		let key = AeadKey([42u8; 32]);
		let base = [9u8; 12];
		let tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base)
			.with_rekey_interval(threshold)
			.with_direction_id(1);
		let rx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey([42u8; 32]), base)
			.with_direction_id(1);
		Self { tx, rx }
	}

	/// Build a pair with a bytes-based rekey threshold on the sender.
	pub fn new_with_bytes_threshold(bytes: u64) -> Self {
		let key = AeadKey([42u8; 32]);
		let base = [9u8; 12];
		let tx = AeadSession::new(AeadSuite::ChaCha20Poly1305, key, base)
			.with_rekey_interval(u64::MAX)
			.with_rekey_bytes_interval(bytes)
			.with_direction_id(1);
		let rx = AeadSession::new(AeadSuite::ChaCha20Poly1305, AeadKey([42u8; 32]), base)
			.with_direction_id(1);
		Self { tx, rx }
	}

	/// Send one message and open it on the receiver.
	pub fn send_roundtrip(&mut self, aad: &[u8], pt: &[u8]) -> Vec<u8> {
		let (seq, ct) = self.tx.seal_next(aad, pt).expect("seal");
		self.rx.open_at(seq, aad, &ct).expect("open")
	}
}

