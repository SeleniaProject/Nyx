use nyx_sdk::error::NyxError;

#[test]
fn protocol_code_passthrough() {
    let e = NyxError::Protocol { message: "Server said nope".into(), code: Some(0x07) }; // pretend UNSUPPORTED_CAP
    assert_eq!(e.close_code(), Some(0x07));
}

#[test]
fn unsupported_cap_in_message_detected() {
    let e = NyxError::Protocol { message: "UNSUPPORTED_CAP capability id=42".into(), code: None };
    assert_eq!(e.close_code(), Some(0x07));
}

#[test]
fn non_protocol_no_code() {
    let e = NyxError::Timeout { duration: std::time::Duration::from_secs(1) };
    assert_eq!(e.close_code(), None);
}

#[test]
fn internal_maps_to_internal_error() {
    let e = NyxError::Internal { message: "boom".into() };
    assert_eq!(e.close_code(), Some(0x06));
}
