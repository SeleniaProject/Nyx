#![cfg(feature = "classic")]

#[test]
fn test_hybrid_message_too_short() {
    let err_result = nyx_crypto::noise::validate_hybrid_message_len(&[0u8; 7]).unwrap_err();
    let s_result = format!("{err_result}");
    assert!(s_result.contains("below minimum"));
}
