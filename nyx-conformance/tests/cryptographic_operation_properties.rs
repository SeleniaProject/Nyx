#[test]
fn hybrid_message_len_minimum() {
	let __err = nyx_crypto::noise::validate_hybrid_message_len(&[1,2,3,4,5,6,7]).unwrap_err();
	let __s = format!("{err}");
	assert!(_s.contains("too short"));
}

