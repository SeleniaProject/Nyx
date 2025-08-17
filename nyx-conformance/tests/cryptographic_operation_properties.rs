#[test]
fn hybrid_message_len_minimum() {
	let err = nyx_crypto::noise::validate_hybrid_message_len(&[1,2,3,4,5,6,7]).unwrap_err();
	let s = format!("{err}");
	assert!(s.contains("too short"));
}

