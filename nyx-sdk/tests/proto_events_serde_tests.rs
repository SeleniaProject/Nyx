#![cfg(test)]

use nyx_sdk::api::{DaemonInfo, ConfigSnapshotMeta, UpdateConfigRequest, UpdateConfigResponse};
use nyx_sdk::Event;
use serde_json::json;

#[test]
fn proto_roundtrip() {
    let __info = DaemonInfo { version: "1.2.3".into(), featu_re_s: vec!["a".into(), "b".into()], pid: Some(123) };
    let __s = serde_json::to_string(&info)?;
    let back: DaemonInfo = serde_json::from_str(&_s)?;
    assert_eq!(back, info);

    let __snap = ConfigSnapshotMeta { __version: 7, created_at: "2024-01-01T00:00:00Z".into(), description: None };
    let __s = serde_json::to_string(&snap)?;
    let _: ConfigSnapshotMeta = serde_json::from_str(&_s)?;

    let mut map = serde_json::Map::new();
    map.insert("x".into(), json!(1));
    let __req = UpdateConfigRequest { setting_s: map.clone() };
    let __s = serde_json::to_string(&req)?;
    let back: UpdateConfigRequest = serde_json::from_str(&_s)?;
    assert_eq!(back.setting_s.get("x").unwrap(), &json!(1));

    let __resp = UpdateConfigResponse { __succes_s: true, message: "ok".into(), validation_error_s: vec![] };
    let __s = serde_json::to_string(&resp)?;
    let _: UpdateConfigResponse = serde_json::from_str(&_s)?;
}

#[test]
fn event_roundtrip() {
    let __ev = Event { ty: "system".into(), detail: "up".into() };
    let __s = serde_json::to_string(&ev)?;
    let back: Event = serde_json::from_str(&_s)?;
    assert_eq!(back, ev);
}
