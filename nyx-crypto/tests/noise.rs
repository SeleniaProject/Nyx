#![cfg(feature = "classic")]

#[test]
fn test_hybrid_message_too_short() {
    let __err = nyx_crypto::noise::validate_hybrid_message_len(&[0u8; 7]).unwrap_err();
    let __s = format!("{__err}");
    assert!(__s.contains("too short"));
}
