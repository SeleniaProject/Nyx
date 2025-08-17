//! HPKE rekey integration tests (exercise counters and rekey path via AeadSession).

use nyx_stream::hpke_rekey::RekeyHarness;

#[test]
fn hpke_rekey_triggers_on_packet_threshold() {
	// Set threshold to 1: after one record, the sender should request rekey.
	let mut h = RekeyHarness::new_with_record_threshold(1);
	let rt0 = h.send_roundtrip(b"aad", b"hello");
	assert_eq!(rt0, b"hello");
	assert!(h.tx.needs_rekey());
	// Perform rekey on both ends and ensure new data still roundtrips with seq reset
	h.tx.rekey();
	h.rx.rekey();
	assert_eq!(h.tx.seq(), 0);
	let rt1 = h.send_roundtrip(b"aad", b"world");
	assert_eq!(rt1, b"world");
}

#[test]
fn hpke_rekey_by_bytes_threshold() {
	// Simulate bytes-based trigger by sending messages until threshold.
	let mut h = RekeyHarness::new_with_bytes_threshold(20);
	assert!(!h.tx.needs_rekey());
	let _ = h.send_roundtrip(b"a", b"hello");
	assert!(h.tx.needs_rekey());
}
