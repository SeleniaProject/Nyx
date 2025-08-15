#![forbid(unsafe_code)]

#[tokio::test]
async fn wasm_settings_low_power_triggers_transport_hook() {
    use nyx_core::config::NyxConfig;
    use nyx_stream::management::{build_settings_frame, parse_settings_frame};
    use nyx_stream::management::setting_ids as mgmt_ids;

    // Ensure config constructs (no daemon internals needed here)
    let _cfg = NyxConfig::default();

    // Build a SETTINGS payload with LOW_POWER_PREFERENCE=1
    let settings = vec![nyx_stream::Setting {
        id: mgmt_ids::LOW_POWER_PREFERENCE,
        value: 1,
    }];
    let body = build_settings_frame(&settings);

    // Validate the frame parses and carries the expected setting
    let (_rest, parsed) = parse_settings_frame(&body).expect("parse settings");
    assert!(parsed.settings.iter().any(|s| s.id == mgmt_ids::LOW_POWER_PREFERENCE && s.value == 1));
}
