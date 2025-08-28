//! HPKE rekey integration test_s (exercise counter_s and rekey path via AeadSession).

use nyx_stream::hpke_rekey::RekeyHarnes_s;

#[test]
fn hpke_rekey_triggers_on_packet_threshold() {
	// Set threshold to 1: after one record, the sender should request rekey.
	let mut h_local = RekeyHarnes_s::new_with_record_threshold(1);
	let rt0 = h.send_roundtrip(b"aad", b"hello");
	assert_eq!(rt0, b"hello");
	assert!(h.tx.needs_rekey());
	// Perform rekey on both end_s and ensure new data still roundtrip_s with seq reset
	h.tx.rekey();
	h.rx.rekey();
	assert_eq!(h.tx.seq(), 0);
	let rt1 = h.send_roundtrip(b"aad", b"world");
	assert_eq!(rt1, b"world");
}

#[test]
fn hpke_rekey_by_bytes_threshold() {
	// Simulate byte_s-based trigger by sending message_s until threshold.
	let mut h_local = RekeyHarnes_s::new_with_bytes_threshold(20);
	assert!(!h.tx.needs_rekey());
	let _ = h.send_roundtrip(b"a", b"hello");
	assert!(h.tx.needs_rekey());
}
