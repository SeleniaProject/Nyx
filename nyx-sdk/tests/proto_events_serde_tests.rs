#![cfg(test)]

use nyx_sdk::api::{ConfigSnapshotMeta, DaemonInfo, UpdateConfigRequest, UpdateConfigResponse};
use nyx_sdk::Event;
use serde_json::json;

#[test]
fn proto_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let info = DaemonInfo {
        __version: "1.2.3".into(),
        featu_re_s: vec!["a".into(), "b".into()],
        pid: Some(123),
    };
    let s = serde_json::to_string(&info)?;
    let back: DaemonInfo = serde_json::from_str(&s)?;
    assert_eq!(back, info);

    let snap = ConfigSnapshotMeta {
        __version: 7,
        __created_at: "2024-01-01T00:00:00Z".into(),
        description: None,
    };
    let s = serde_json::to_string(&snap)?;
    let _: ConfigSnapshotMeta = serde_json::from_str(&s)?;

    let mut map = serde_json::Map::new();
    map.insert("x".into(), json!(1));
    let req = UpdateConfigRequest {
        setting_s: map.clone(),
    };
    let s = serde_json::to_string(&req)?;
    let back: UpdateConfigRequest = serde_json::from_str(&s)?;
    assert_eq!(back.setting_s.get("x").unwrap(), &json!(1));

    let resp = UpdateConfigResponse {
        __succes_s: true,
        __message: "ok".into(),
        validation_error_s: vec![],
    };
    let s = serde_json::to_string(&resp)?;
    let _: UpdateConfigResponse = serde_json::from_str(&s)?;
    Ok(())
}

#[test]
fn event_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
    let ev = Event {
        event_type: "system".into(),
        detail: "up".into(),
    };
    let s = serde_json::to_string(&ev)?;
    let back: Event = serde_json::from_str(&s)?;
    assert_eq!(back, ev);
    Ok(())
}
