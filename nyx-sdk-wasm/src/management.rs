//! WASM-side helpers to build Nyx management frames (SETTINGS, CLOSE) for browser clients.
use base64::engine::{general_purpose, Engine};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, Headers, Request, Response};

#[derive(Debug, Clone, Copy)]
struct Setting {
    id: u16,
    value: u32,
}

fn build_settings_frame(settings: &[Setting]) -> Vec<u8> {
    let mut v = Vec::with_capacity(settings.len() * 6);
    for s in settings {
        v.extend_from_slice(&s.id.to_be_bytes());
        v.extend_from_slice(&s.value.to_be_bytes());
    }
    v
}

/// SETTINGS IDs (subset) matching nyx-stream/src/management.rs
mod setting_ids {
    pub const PLUGIN_SUPPORT: u16 = 0x0010;
    pub const PLUGIN_REQUIRED: u16 = 0x0011;
    #[allow(dead_code)] // reserved for plugin negotiation flags
    pub const PLUGIN_OPTIONAL: u16 = 0x0012;
    pub const PLUGIN_SECURITY_POLICY: u16 = 0x0013;
}

/// Plugin support flags (must match nyx-stream)
pub mod plugin_support_flags {
    pub const BASIC_FRAMES: u32 = 0x0001;
    pub const DYNAMIC_LOADING: u32 = 0x0002;
    pub const SANDBOXED_EXECUTION: u32 = 0x0004;
    pub const INTER_PLUGIN_IPC: u32 = 0x0008;
    pub const PLUGIN_PERSISTENCE: u32 = 0x0010;
}

/// Plugin security policy flags (must match nyx-stream)
pub mod plugin_security_flags {
    pub const REQUIRE_SIGNATURES: u32 = 0x0001;
    pub const ALLOW_NETWORK: u32 = 0x0002;
    pub const ALLOW_FILESYSTEM: u32 = 0x0004;
    pub const ALLOW_INTER_PLUGIN_IPC: u32 = 0x0008;
    pub const ALLOW_PROCESS_SPAWN: u32 = 0x0010;
}

/// Build a SETTINGS payload carrying plugin support + security policy + required plugin count.
/// `required_cbor_b64` is a base64url CBOR array<u32> as produced by `nyx_plugin_required_cbor_b64`.
#[wasm_bindgen]
pub fn nyx_build_plugin_settings(
    required_cbor_b64: String,
    support_flags: u32,
    security_policy: u32,
) -> Result<js_sys::Uint8Array, JsValue> {
    // Count required IDs by decoding CBOR
    let cbor = general_purpose::URL_SAFE_NO_PAD
        .decode(required_cbor_b64.as_bytes())
        .map_err(|e| JsValue::from_str(&format!("base64 decode error: {}", e)))?;
    let ids: Vec<u32> = ciborium::from_reader(cbor.as_slice())
        .map_err(|e| JsValue::from_str(&format!("CBOR decode error: {}", e)))?;
    let count = ids.len() as u32;

    let mut settings = Vec::new();
    settings.push(Setting {
        id: setting_ids::PLUGIN_SUPPORT,
        value: support_flags,
    });
    settings.push(Setting {
        id: setting_ids::PLUGIN_SECURITY_POLICY,
        value: security_policy,
    });
    settings.push(Setting {
        id: setting_ids::PLUGIN_REQUIRED,
        value: count,
    });

    let bytes = build_settings_frame(&settings);
    Ok(js_sys::Uint8Array::from(bytes.as_slice()))
}

/// Build CLOSE payload for unsupported capability (code=0x07) with 4-byte cap_id reason body.
#[wasm_bindgen]
pub fn nyx_build_close_unsupported_cap(cap_id: u32) -> js_sys::Uint8Array {
    let code: u16 = 0x0007; // ERR_UNSUPPORTED_CAP
    let mut v = Vec::with_capacity(2 + 1 + 4);
    v.extend_from_slice(&code.to_be_bytes());
    v.push(4u8);
    v.extend_from_slice(&cap_id.to_be_bytes());
    js_sys::Uint8Array::from(v.as_slice())
}

/// Compute plugin support flags enabling BASIC_FRAMES by default and adding requested features.
#[wasm_bindgen]
pub fn nyx_compute_plugin_support_flags(
    dynamic_loading: bool,
    sandboxed_execution: bool,
    inter_plugin_ipc: bool,
    persistence: bool,
) -> u32 {
    let mut flags = plugin_support_flags::BASIC_FRAMES;
    if dynamic_loading {
        flags |= plugin_support_flags::DYNAMIC_LOADING;
    }
    if sandboxed_execution {
        flags |= plugin_support_flags::SANDBOXED_EXECUTION;
    }
    if inter_plugin_ipc {
        flags |= plugin_support_flags::INTER_PLUGIN_IPC;
    }
    if persistence {
        flags |= plugin_support_flags::PLUGIN_PERSISTENCE;
    }
    flags
}

/// Compute plugin security policy flags.
#[wasm_bindgen]
pub fn nyx_compute_plugin_security_policy(
    require_signatures: bool,
    allow_network: bool,
    allow_filesystem: bool,
    allow_inter_plugin_ipc: bool,
    allow_process_spawn: bool,
) -> u32 {
    let mut policy = 0u32;
    if require_signatures {
        policy |= plugin_security_flags::REQUIRE_SIGNATURES;
    }
    if allow_network {
        policy |= plugin_security_flags::ALLOW_NETWORK;
    }
    if allow_filesystem {
        policy |= plugin_security_flags::ALLOW_FILESYSTEM;
    }
    if allow_inter_plugin_ipc {
        policy |= plugin_security_flags::ALLOW_INTER_PLUGIN_IPC;
    }
    if allow_process_spawn {
        policy |= plugin_security_flags::ALLOW_PROCESS_SPAWN;
    }
    policy
}

#[derive(Serialize)]
struct PluginHandshakeBundle {
    required_cbor_b64: String,
    support_flags: u32,
    security_policy: u32,
    settings: String, // base64url-encoded SETTINGS payload
}

/// Prepare a bundle for plugin handshake: required list (CBOR b64), flags, and SETTINGS payload (b64url).
#[wasm_bindgen]
pub fn nyx_prepare_plugin_handshake(
    required_cbor_b64: String,
    support_flags: u32,
    security_policy: u32,
) -> Result<JsValue, JsValue> {
    let settings =
        nyx_build_plugin_settings(required_cbor_b64.clone(), support_flags, security_policy)?;
    let settings_b64 = general_purpose::URL_SAFE_NO_PAD.encode(settings.to_vec());
    let bundle = PluginHandshakeBundle {
        required_cbor_b64: required_cbor_b64,
        support_flags,
        security_policy,
        settings: settings_b64,
    };
    serde_wasm_bindgen::to_value(&bundle).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// POST the plugin SETTINGS payload to a gateway URL. Returns a JSON {status, ok}.
#[wasm_bindgen]
pub async fn nyx_send_plugin_settings(
    url: String,
    required_cbor_b64: String,
    support_flags: u32,
    security_policy: u32,
) -> Result<JsValue, JsValue> {
    let settings = nyx_build_plugin_settings(required_cbor_b64, support_flags, security_policy)?;
    let headers = Headers::new().map_err(|e| JsValue::from(e))?;
    headers
        .append("Content-Type", "application/nyx-settings")
        .map_err(|e| JsValue::from(e))?;
    let win = window().ok_or(JsValue::from_str("no window"))?;
    // Construct a Request via JS constructor using Reflect for compatibility
    let init = js_sys::Object::new();
    js_sys::Reflect::set(
        &init,
        &JsValue::from_str("method"),
        &JsValue::from_str("POST"),
    )?;
    js_sys::Reflect::set(&init, &JsValue::from_str("body"), &settings)?;
    let request_ctor = js_sys::Function::new_with_args("u,i", "return new Request(u,i);");
    let request_js = request_ctor.call2(&JsValue::NULL, &JsValue::from_str(&url), &init)?;
    let request: Request = request_js.dyn_into()?;
    request
        .headers()
        .set("Content-Type", "application/nyx-settings")?;
    let resp_value = JsFuture::from(win.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;
    let status = resp.status();
    let ok = resp.ok();
    let out = serde_json::json!({"status": status, "ok": ok});
    serde_wasm_bindgen::to_value(&out).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// POST a CLOSE payload (0x3F) to a gateway URL. Returns a JSON {status, ok}.
#[wasm_bindgen]
pub async fn nyx_send_close(
    url: String,
    close_payload: js_sys::Uint8Array,
) -> Result<JsValue, JsValue> {
    let headers = Headers::new().map_err(|e| JsValue::from(e))?;
    headers
        .append("Content-Type", "application/nyx-close")
        .map_err(|e| JsValue::from(e))?;
    let win = window().ok_or(JsValue::from_str("no window"))?;
    let init = js_sys::Object::new();
    js_sys::Reflect::set(
        &init,
        &JsValue::from_str("method"),
        &JsValue::from_str("POST"),
    )?;
    js_sys::Reflect::set(&init, &JsValue::from_str("body"), &close_payload)?;
    let request_ctor = js_sys::Function::new_with_args("u,i", "return new Request(u,i);");
    let request_js = request_ctor.call2(&JsValue::NULL, &JsValue::from_str(&url), &init)?;
    let request: Request = request_js.dyn_into()?;
    request
        .headers()
        .set("Content-Type", "application/nyx-close")?;
    let resp_value = JsFuture::from(win.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;
    let status = resp.status();
    let ok = resp.ok();
    let out = serde_json::json!({"status": status, "ok": ok});
    serde_wasm_bindgen::to_value(&out).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// GET the last plugin negotiation snapshot from daemon gateway
#[wasm_bindgen]
pub async fn nyx_get_plugin_negotiation(url: String) -> Result<JsValue, JsValue> {
    let win = window().ok_or(JsValue::from_str("no window"))?;
    // Build Request for GET
    let request_ctor = js_sys::Function::new_with_args("u", "return new Request(u);");
    let request_js = request_ctor.call1(&JsValue::NULL, &JsValue::from_str(&url))?;
    let request: Request = request_js.dyn_into()?;
    let resp_value = JsFuture::from(win.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;
    // Parse JSON response
    let json_promise = resp.json().map_err(|e| JsValue::from(e))?;
    let json = JsFuture::from(json_promise).await?;
    Ok(json)
}

/// POST to start plugin handshake at the daemon gateway and get initial SETTINGS (base64url) if any
#[wasm_bindgen]
pub async fn nyx_start_plugin_handshake(url: String) -> Result<JsValue, JsValue> {
    let win = window().ok_or(JsValue::from_str("no window"))?;
    // Construct POST request without body
    let init = js_sys::Object::new();
    js_sys::Reflect::set(
        &init,
        &JsValue::from_str("method"),
        &JsValue::from_str("POST"),
    )?;
    let request_ctor = js_sys::Function::new_with_args("u,i", "return new Request(u,i);");
    let request_js = request_ctor.call2(&JsValue::NULL, &JsValue::from_str(&url), &init)?;
    let request: Request = request_js.dyn_into()?;
    let resp_value = JsFuture::from(win.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;
    let json_promise = resp.json().map_err(|e| JsValue::from(e))?;
    let json = JsFuture::from(json_promise).await?;
    Ok(json)
}

/// POST the peer's SETTINGS to the daemon gateway for processing. Returns optional response SETTINGS (b64url).
#[wasm_bindgen]
pub async fn nyx_process_peer_settings(
    url: String,
    peer_settings: js_sys::Uint8Array,
) -> Result<JsValue, JsValue> {
    let win = window().ok_or(JsValue::from_str("no window"))?;
    let init = js_sys::Object::new();
    js_sys::Reflect::set(
        &init,
        &JsValue::from_str("method"),
        &JsValue::from_str("POST"),
    )?;
    js_sys::Reflect::set(&init, &JsValue::from_str("body"), &peer_settings)?;
    let request_ctor = js_sys::Function::new_with_args("u,i", "return new Request(u,i);");
    let request_js = request_ctor.call2(&JsValue::NULL, &JsValue::from_str(&url), &init)?;
    let request: Request = request_js.dyn_into()?;
    request
        .headers()
        .set("Content-Type", "application/nyx-settings")?;
    let resp_value = JsFuture::from(win.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;
    let json_promise = resp.json().map_err(|e| JsValue::from(e))?;
    let json = JsFuture::from(json_promise).await?;
    Ok(json)
}

/// POST to complete the plugin handshake and get result summary
#[wasm_bindgen]
pub async fn nyx_complete_plugin_handshake(url: String) -> Result<JsValue, JsValue> {
    let win = window().ok_or(JsValue::from_str("no window"))?;
    let init = js_sys::Object::new();
    js_sys::Reflect::set(
        &init,
        &JsValue::from_str("method"),
        &JsValue::from_str("POST"),
    )?;
    let request_ctor = js_sys::Function::new_with_args("u,i", "return new Request(u,i);");
    let request_js = request_ctor.call2(&JsValue::NULL, &JsValue::from_str(&url), &init)?;
    let request: Request = request_js.dyn_into()?;
    let resp_value = JsFuture::from(win.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;
    let json_promise = resp.json().map_err(|e| JsValue::from(e))?;
    let json = JsFuture::from(json_promise).await?;
    Ok(json)
}

/// Convenience: process peer settings (b64url) then complete handshake against daemon gateway
#[wasm_bindgen]
pub async fn nyx_autopilot_process_and_complete(
    base_url: String,
    peer_settings_b64: String,
) -> Result<JsValue, JsValue> {
    // Decode peer settings from base64url to Uint8Array
    let bytes = general_purpose::URL_SAFE_NO_PAD
        .decode(peer_settings_b64.as_bytes())
        .map_err(|e| JsValue::from_str(&format!("base64url decode error: {}", e)))?;
    let u8a = js_sys::Uint8Array::from(bytes.as_slice());
    // POST process-peer-settings
    let process_url = format!(
        "{}/api/v1/wasm/handshake/process-peer-settings",
        base_url.trim_end_matches('/')
    );
    let _ = nyx_process_peer_settings(process_url, u8a).await?;
    // POST complete
    let complete_url = format!(
        "{}/api/v1/wasm/handshake/complete",
        base_url.trim_end_matches('/')
    );
    nyx_complete_plugin_handshake(complete_url).await
}

/// POST the required plugin list (CBOR b64url) to daemon gateway
#[wasm_bindgen]
pub async fn nyx_set_required_plugins(
    base_url: String,
    required_cbor_b64: String,
) -> Result<JsValue, JsValue> {
    let win = window().ok_or(JsValue::from_str("no window"))?;
    // Build JSON body { required_cbor_b64: "..." }
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(
        &obj,
        &JsValue::from_str("required_cbor_b64"),
        &JsValue::from_str(&required_cbor_b64),
    )?;
    let json = js_sys::JSON::stringify(&obj).map_err(|e| JsValue::from(e))?;
    let init = js_sys::Object::new();
    js_sys::Reflect::set(
        &init,
        &JsValue::from_str("method"),
        &JsValue::from_str("POST"),
    )?;
    js_sys::Reflect::set(&init, &JsValue::from_str("body"), &json)?;
    let request_ctor = js_sys::Function::new_with_args("u,i", "return new Request(u,i);");
    let url = format!(
        "{}/api/v1/wasm/handshake/required",
        base_url.trim_end_matches('/')
    );
    let request_js = request_ctor.call2(&JsValue::NULL, &JsValue::from_str(&url), &init)?;
    let request: Request = request_js.dyn_into()?;
    request.headers().set("Content-Type", "application/json")?;
    let resp_value = JsFuture::from(win.fetch_with_request(&request)).await?;
    let resp: Response = resp_value.dyn_into()?;
    let json_promise = resp.json().map_err(|e| JsValue::from(e))?;
    let out = JsFuture::from(json_promise).await?;
    Ok(out)
}
