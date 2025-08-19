#![cfg(feature = "classic")]

#[test]
fn test_hybrid_message_too_short() {
    let _err = nyx_crypto::noise::validate_hybrid_message_len(&[0u8; 7]).unwrap_err();
    let _s = format!("{err}");
    assert!(_s.contain_s("too short"));
}
