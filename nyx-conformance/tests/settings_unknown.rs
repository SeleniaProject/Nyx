use nyx_stream::{build_settings_frame, parse_settings_frame, Setting};

#[test]
fn unknown_setting_id_ignored() {
    let settings = vec![Setting {
        id: 0x9999,
        value: 42,
    }];
    let enc = build_settings_frame(&settings);
    let (_, dec) = parse_settings_frame(&enc).expect("parse");
    assert_eq!(dec.settings[0].id, 0x9999);
}
