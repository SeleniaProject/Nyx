//! WASM bindings exposing a growing subset of Nyx capabilities.
//!
//! Feature parity notes:
//! - HPKE: Public API surface is planned; handshake demo currently uses classic Noise. HPKE exposure will use wasm-safe RNG and KEM bindings when stabilized.
//! - Multipath & Plugin system: Control/query APIs will be provided for browser clients; transport is limited by browser networking constraints.
//! - Capability negotiation / Close codes: Will be exposed as structured JS errors; interim maps to exceptions.
//! - Push notifications: `nyx_register_push` is provided. Integration with Nyx gateway (VAPID/endpoint exchange) follows WebPush best practices.
use wasm_bindgen::prelude::*;
#[cfg(feature = "noise")]
use nyx_crypto::noise::{initiator_generate, responder_process, initiator_finalize, derive_session_key};
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, PushSubscriptionOptionsInit, ServiceWorkerRegistration, PushSubscription, PushManager};
use js_sys::{Uint8Array, JSON, Object, Reflect};
use base64::engine::{general_purpose, Engine};
mod multipath;
mod plugin;
mod errors;
mod management;
#[cfg(feature = "hpke")]
mod hpke;

pub use multipath::{MultipathController, MultipathConfigWasm, PathStatsWasm, PathSelectionResult};
pub use plugin::{PluginRegistryWasm};
#[cfg(feature = "hpke")]
pub use hpke::{hpke_generate_keypair, hpke_open, hpke_seal, hpke_generate_and_seal_session, hpke_open_session};
pub use errors::{nyx_map_close_code, nyx_check_required_plugins};
pub use management::{nyx_build_plugin_settings, nyx_build_close_unsupported_cap};

#[cfg(feature = "noise")]
#[wasm_bindgen]
pub fn noise_handshake_demo() -> String {
    // Simple demo performing Noise_Nyx X25519 handshake in wasm.
    let (init_pub, init_sec) = initiator_generate();
    let (resp_pub, shared_resp) = responder_process(&init_pub);
    let shared_init = initiator_finalize(init_sec, &resp_pub);
    assert_eq!(shared_init.as_bytes(), shared_resp.as_bytes());
    let key = derive_session_key(&shared_init);
    hex::encode(key.0)
}

/// Register a Service Worker at `sw_path` and subscribe to WebPush with the given VAPID public key.
/// Returns the JSON serialized subscription (to be sent to Nyx gateway).
///
/// This function is `async` in JS; use like:
/// `nyx_register_push("/nyx_sw.js", vapid_key).then(sub => { ... });`
#[wasm_bindgen]
pub async fn nyx_register_push(sw_path: String, vapid_public_key: String) -> Result<JsValue, JsValue> {
    let win = window().ok_or("no window")?;
    let navigator = win.navigator();
    let sw_container = navigator.service_worker();

    // Register service worker if not already controlling.
    let reg_promise = sw_container.register(&sw_path);
    let reg_js = JsFuture::from(reg_promise).await?;
    let reg: ServiceWorkerRegistration = reg_js.dyn_into()?;

    let push: PushManager = reg.push_manager()?;

    // Convert base64 public key to Uint8Array (assumes urlsafe base64 without padding).
    let key_buf = general_purpose::URL_SAFE_NO_PAD.decode(vapid_public_key.as_bytes()).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let key_u8 = Uint8Array::from(&key_buf[..]);
    let key_js: JsValue = key_u8.into();

    let sub_opts = PushSubscriptionOptionsInit::new();
    sub_opts.set_user_visible_only(true);
    sub_opts.set_application_server_key(&key_js);

    let sub_promise = push.subscribe_with_options(&sub_opts).map_err(|e| e)?;
    let sub_js = JsFuture::from(sub_promise).await?;
    let sub: PushSubscription = sub_js.dyn_into()?;

    // Build comprehensive subscription JSON: endpoint + keys (p256dh, auth)
    let js_obj = Object::new();
    Reflect::set(&js_obj, &"endpoint".into(), &sub.endpoint().into())?;
    // keys (via JS getKey fallback to support older web-sys)
    let keys_obj = Object::new();
    let get_key_fn = Reflect::get(&sub, &JsValue::from_str("getKey"))?;
    if let Some(f) = get_key_fn.dyn_ref::<js_sys::Function>() {
        // p256dh
        let res = f.call1(&sub, &JsValue::from_str("p256dh"))?;
        if !res.is_undefined() && !res.is_null() {
            let u8 = js_sys::Uint8Array::new(&res);
            let v = u8.to_vec();
            let b64 = general_purpose::URL_SAFE_NO_PAD.encode(v);
            Reflect::set(&keys_obj, &"p256dh".into(), &JsValue::from_str(&b64))?;
        }
        // auth
        let res = f.call1(&sub, &JsValue::from_str("auth"))?;
        if !res.is_undefined() && !res.is_null() {
            let u8 = js_sys::Uint8Array::new(&res);
            let v = u8.to_vec();
            let b64 = general_purpose::URL_SAFE_NO_PAD.encode(v);
            Reflect::set(&keys_obj, &"auth".into(), &JsValue::from_str(&b64))?;
        }
    }
    Reflect::set(&js_obj, &"keys".into(), &keys_obj.into())?;
    
    let json_str = JSON::stringify(&js_obj)?;
    Ok(json_str.into())
} 

/// Multipath controller factory (convenience for JS users)
#[wasm_bindgen]
pub fn nyx_multipath_controller_new(config_json: Option<String>) -> MultipathController {
    let cfg = config_json.map(|s| MultipathConfigWasm::new(Some(s)).unwrap_or_else(|_| MultipathConfigWasm::new(None).unwrap()));
    MultipathController::new(cfg)
}

/// Create an empty plugin registry for client-side manifest management
#[wasm_bindgen]
pub fn nyx_plugin_registry_new() -> PluginRegistryWasm {
    PluginRegistryWasm::new()
}

/// Convenience: export required plugin IDs as CBOR (base64url) from the registry
#[wasm_bindgen]
pub fn nyx_plugin_required_cbor_b64(registry: &PluginRegistryWasm) -> Result<String, JsValue> {
    registry.export_required_plugins_cbor_b64()
}