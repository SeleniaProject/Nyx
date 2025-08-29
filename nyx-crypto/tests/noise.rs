#![cfg(feature = "classic")]
#![allow(missing_docs, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[test]
fn test_hybrid_message_too_short() {
    match nyx_crypto::noise::validate_hybrid_message_len(&[0u8; 7]) {
        Err(e) => assert!(format!("{e}").contains("below minimum")),
        Ok(_) => panic!("expected error"),
    }
}
