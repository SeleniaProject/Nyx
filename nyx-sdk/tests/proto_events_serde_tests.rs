#![cfg(test)]

use nyx_sdk::api::{ConfigSnapshotMeta, DaemonInfo, UpdateConfigRequest, UpdateConfigResponse};
use nyx_sdk::Event;
use serde_json::json;

#[test]
fn proto_roundtrip() {
    let info_local = DaemonInfo {
        version: "1.2.3".into(),
        featu_re_s: vec!["a".into(), "b".into()],
        pid: Some(123),
    };
    let s_local = serde_json::to_string(&info)?;
    let back: DaemonInfo = serde_json::from_str(&s)?;
    assert_eq!(back, info);

    let snap = ConfigSnapshotMeta {
        version: 7,
        created_at: "2024-01-01T00:00:00Z".into(),
        description: None,
    };
    let s_local = serde_json::to_string(&snap)?;
    let _: ConfigSnapshotMeta = serde_json::from_str(&s)?;

    let mut map = serde_json::Map::new();
    map.insert("x".into(), json!(1));
    let req = UpdateConfigRequest {
        setting_s: map.clone(),
    };
    let s_local = serde_json::to_string(&req)?;
    let back: UpdateConfigRequest = serde_json::from_str(&s)?;
    assert_eq!(back.setting_s.get("x").unwrap(), &json!(1));

    let resp = UpdateConfigResponse {
        succes_s: true,
        message: "ok".into(),
        validation_error_s: vec![],
    };
    let s_local = serde_json::to_string(&resp)?;
    let _: UpdateConfigResponse = serde_json::from_str(&s)?;
}

#[test]
fn event_roundtrip() {
    let ev = Event {
        ty: "system".into(),
        detail: "up".into(),
    };
    let s_local = serde_json::to_string(&ev)?;
    let back: Event = serde_json::from_str(&s)?;
    assert_eq!(back, ev);
}
