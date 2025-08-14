//! WASM-facing structured error mapping and validation helpers.
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use base64::engine::{general_purpose, Engine};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NyxErrorKind {
    Unknown,
    Protocol,
    UnsupportedCapability,
    Permission,
    Network,
    Timeout,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NyxSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NyxErrorWasm {
    pub code: Option<u16>,
    pub kind: NyxErrorKind,
    pub severity: NyxSeverity,
    pub message: String,
    pub details: serde_json::Value,
}

fn unsupported_capability(message: impl Into<String>, details: serde_json::Value) -> NyxErrorWasm {
    NyxErrorWasm {
        code: Some(0x07),
        kind: NyxErrorKind::UnsupportedCapability,
        severity: NyxSeverity::High,
        message: message.into(),
        details,
    }
}

#[wasm_bindgen]
pub fn nyx_map_close_code(code: u16) -> Result<JsValue, JsValue> {
    let err = match code {
        0x07 => unsupported_capability(
            "Required capability or plugin is not supported by peer",
            serde_json::json!({"category":"unsupported_capability"})
        ),
        _ => NyxErrorWasm {
            code: Some(code),
            kind: NyxErrorKind::Protocol,
            severity: NyxSeverity::Medium,
            message: format!("Protocol close code 0x{:02X}", code),
            details: serde_json::json!({}),
        }
    };
    serde_wasm_bindgen::to_value(&err).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Validate that all required plugin IDs (CBOR base64url) are present in the given supported IDs (JSON array).
/// Returns Ok(()) if satisfied; else returns a structured JS error.
#[wasm_bindgen]
pub fn nyx_check_required_plugins(required_cbor_b64: String, supported_ids_json: String) -> Result<(), JsValue> {
    // Decode base64url CBOR
    let cbor_bytes = general_purpose::URL_SAFE_NO_PAD
        .decode(required_cbor_b64.as_bytes())
        .map_err(|e| JsValue::from_str(&format!("base64 decode error: {}", e)))?;
    // Decode CBOR array<u32>
    let required: Vec<u32> = ciborium::from_reader(cbor_bytes.as_slice())
        .map_err(|e| JsValue::from_str(&format!("CBOR decode error: {}", e)))?;
    // Parse supported IDs JSON (array of numbers)
    let supported: Vec<u32> = serde_json::from_str(&supported_ids_json)
        .map_err(|e| JsValue::from_str(&format!("supported_ids_json parse error: {}", e)))?;
    let supported_set: std::collections::HashSet<u32> = supported.into_iter().collect();
    let missing: Vec<u32> = required.into_iter().filter(|id| !supported_set.contains(id)).collect();
    if missing.is_empty() {
        return Ok(());
    }
    let err = unsupported_capability(
        "Missing required plugins",
        serde_json::json!({"missing": missing})
    );
    Err(serde_wasm_bindgen::to_value(&err).map_err(|e| JsValue::from_str(&e.to_string()))?)
}


