#![cfg(target_arch = "wasm32")]
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use nyx_sdk_wasm::{
    nyx_build_plugin_settings, MultipathConfigWasm, MultipathController, PluginRegistryWasm,
};

#[wasm_bindgen_test]
fn multipath_select_and_history() {
    let cfg = MultipathConfigWasm::new(None).unwrap();
    let mut ctrl = MultipathController::new(Some(cfg));
    ctrl.add_path(1, 10, None).unwrap();
    ctrl.add_path(2, 10, None).unwrap();
    let _sel = ctrl.select_path();
    let hist = ctrl.get_selection_history_json();
    assert!(hist.len() > 2);
}

#[wasm_bindgen_test]
fn plugin_required_settings_roundtrip() {
    let mut reg = PluginRegistryWasm::new();
    let manifest = serde_json::json!({
        "id": 1001u32,
        "name": "demo",
        "version": "1.0.0",
        "description": "demo plugin",
        "permissions": ["receive_frames", "control"],
        "required": true
    });
    reg.add_manifest(manifest.to_string()).unwrap();
    let required = reg.export_required_plugins_cbor_b64().unwrap();
    // Build SETTINGS payload from required list
    let flags: u32 = 0x0001; // BASIC_FRAMES
    let policy: u32 = 0x0001; // REQUIRE_SIGNATURES
    let settings = nyx_build_plugin_settings(required, flags, policy).unwrap();
    assert!(settings.length() > 0);
}
