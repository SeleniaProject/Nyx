#![forbid(unsafe_code)]

#[test]
fn settings_ext_roundtrip_with_cbor() {
    use nyx_stream::management::{
        build_settings_frame_ext, parse_settings_frame_ext, setting_ids, Setting, SettingsFrame,
    };
    // Base TLVs
    let base = vec![
        Setting {
            id: setting_ids::MAX_CONCURRENT_STREAMS,
            value: 8,
        },
        Setting {
            id: setting_ids::MULTIPATH_ENABLED,
            value: 1,
        },
    ];
    // CBOR ext payloads (dummy bytes)
    let req_cbor = vec![0x82, 0x01, 0x02]; // [1,2]
    let opt_cbor = vec![0x81, 0x0A]; // [10]
    let payload = build_settings_frame_ext(
        &base,
        &[
            (setting_ids::PLUGIN_REQUIRED_CBOR, &req_cbor),
            (setting_ids::PLUGIN_OPTIONAL_CBOR, &opt_cbor),
        ],
    );

    let (_rem, (frame, ext)) = parse_settings_frame_ext(&payload).expect("parse ext");
    assert_eq!(frame.settings.len(), 2);
    assert_eq!(ext.len(), 2);
    assert!(ext
        .iter()
        .any(|(id, b)| *id == setting_ids::PLUGIN_REQUIRED_CBOR
            && b.as_slice() == req_cbor.as_slice()));
    assert!(ext
        .iter()
        .any(|(id, b)| *id == setting_ids::PLUGIN_OPTIONAL_CBOR
            && b.as_slice() == opt_cbor.as_slice()));
}
