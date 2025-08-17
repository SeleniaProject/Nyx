#![cfg(test)]

use nyx_sdk::api::{DaemonInfo, ConfigSnapshotMeta, UpdateConfigRequest, UpdateConfigResponse};
use nyx_sdk::Event;
use serde_json::json;

#[test]
fn proto_roundtrip() {
    let info = DaemonInfo { version: "1.2.3".into(), features: vec!["a".into(), "b".into()], pid: Some(123) };
    let s = serde_json::to_string(&info).unwrap();
    let back: DaemonInfo = serde_json::from_str(&s).unwrap();
    assert_eq!(back, info);

    let snap = ConfigSnapshotMeta { version: 7, created_at: "2024-01-01T00:00:00Z".into(), description: None };
    let s = serde_json::to_string(&snap).unwrap();
    let _: ConfigSnapshotMeta = serde_json::from_str(&s).unwrap();

    let mut map = serde_json::Map::new();
    map.insert("x".into(), json!(1));
    let req = UpdateConfigRequest { settings: map.clone() };
    let s = serde_json::to_string(&req).unwrap();
    let back: UpdateConfigRequest = serde_json::from_str(&s).unwrap();
    assert_eq!(back.settings.get("x").unwrap(), &json!(1));

    let resp = UpdateConfigResponse { success: true, message: "ok".into(), validation_errors: vec![] };
    let s = serde_json::to_string(&resp).unwrap();
    let _: UpdateConfigResponse = serde_json::from_str(&s).unwrap();
}

#[test]
fn event_roundtrip() {
    let ev = Event { ty: "system".into(), detail: "up".into() };
    let s = serde_json::to_string(&ev).unwrap();
    let back: Event = serde_json::from_str(&s).unwrap();
    assert_eq!(back, ev);
}
