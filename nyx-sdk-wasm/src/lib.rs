#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// For non-WASM environments, define a placeholder JsValue
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct JsValue(#[allow(dead_code)] String);

#[cfg(not(target_arch = "wasm32"))]
impl JsValue {
    pub fn from_string(s: &str) -> Self {
        JsValue(s.to_string())
    }
}

pub mod errors;
pub mod multipath;
pub mod noise;
pub mod push;

#[cfg(feature = "hpke")]
pub mod hpke;

#[cfg(target_arch = "wasm32")]
pub use multipath::MultipathManager;

use errors::{NyxWasmError, WasmResult};

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn init() {
    // Initialize logging for WASM environment
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::console::log_1(&"Nyx WASM SDK initialized".into());
    }
}

/// Get SDK version information
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Initialize SDK with configuration
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn init_with_config(config: &str) -> WasmResult<()> {
    init();

    #[cfg(target_arch = "wasm32")]
    {
        web_sys::console::log_1(&format!("Nyx SDK initialized with config: {}", config).into());
    }

    // Parse and validate configuration
    let _config: serde_json::Value = serde_json::from_str(config)
        .map_err(|e| NyxWasmError::ConfigurationError(format!("Invalid config JSON: {e}")))?;

    Ok(())
}

/// Perform Noise handshake demonstration for cryptographic showcase
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn noise_handshake_demo(initiator_config: &str, responder_config: &str) -> WasmResult<String> {
    noise::perform_handshake_demo(initiator_config, responder_config)
}

/// Register for push notifications with Nyx Gateway integration
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn nyx_register_push(gateway_url: &str, client_config: &str) -> WasmResult<String> {
    push::register_push_endpoint(gateway_url, client_config)
}

/// Check WASM environment capabilities
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn check_capabilities() -> String {
    let mut capabilities = vec![
        "noise_handshake_demo".to_string(),
        "nyx_register_push".to_string(),
        "multipath_management".to_string(),
    ];

    capabilities.extend(vec![
        #[cfg(feature = "hpke")]
        "hpke_support".to_string(),
        #[cfg(feature = "plugin")]
        "plugin_system".to_string(),
    ]);

    serde_json::to_string(&capabilities).unwrap_or_else(|_| "[]".to_string())
}

/// Check if HPKE is available (conditional compilation)
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn hpke_available() -> bool {
    #[cfg(feature = "hpke")]
    {
        hpke::hpke_available_internal()
    }
    #[cfg(not(feature = "hpke"))]
    {
        false
    }
}

/// HPKE encrypt function (if feature enabled)
#[cfg(feature = "hpke")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn hpke_encrypt(
    recipient_public_key: &str,
    plaintext: &str,
    config_json: Option<String>,
) -> WasmResult<String> {
    hpke::hpke_encrypt_internal(recipient_public_key, plaintext, config_json)
}

/// HPKE decrypt function (if feature enabled)
#[cfg(feature = "hpke")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn hpke_decrypt(
    private_key: &str,
    encapsulated_key: &str,
    ciphertext: &str,
    config_json: Option<String>,
) -> WasmResult<String> {
    hpke::hpke_decrypt_internal(private_key, encapsulated_key, ciphertext, config_json)
}

/// HPKE generate keypair function (if feature enabled)
#[cfg(feature = "hpke")]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn hpke_generate_keypair() -> WasmResult<String> {
    hpke::hpke_generate_keypair_internal()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        init();
        // Basic smoke test - should not panic
    }

    #[test]
    fn test_version() {
        let version = version();
        assert!(!version.is_empty());
    }

    #[test]
    fn test_init_with_valid_config() {
        let config = r#"{"multipath": {"enabled": true}}"#;
        assert!(init_with_config(config).is_ok());
    }

    #[test]
    fn test_init_with_invalid_config() {
        let config = "invalid json";
        assert!(init_with_config(config).is_err());
    }

    #[test]
    fn test_check_capabilities() {
        let caps = check_capabilities();
        assert!(caps.contains("noise_handshake_demo"));
        assert!(caps.contains("nyx_register_push"));
    }

    #[test]
    fn test_noise_handshake_demo() {
        let initiator_config = r#"{
            "pattern": "Noise_XX_25519_ChaChaPoly_BLAKE2s",
            "psk": null,
            "static_keypair": null,
            "payload": "test"
        }"#;

        let responder_config = r#"{
            "pattern": "Noise_XX_25519_ChaChaPoly_BLAKE2s",
            "psk": null,
            "static_keypair": null,
            "payload": "test"
        }"#;

        let result = noise_handshake_demo(initiator_config, responder_config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_nyx_register_push() {
        let gateway_url = "https://gateway.nyx.example.com";
        let client_config = r#"{
            "application_server_key": "test_vapid_key",
            "user_agent": "test_browser"
        }"#;

        let result = nyx_register_push(gateway_url, client_config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hpke_availability() {
        let _ = hpke_available();
        // Should compile regardless of feature state
    }

    #[cfg(feature = "hpke")]
    #[test]
    fn test_hpke_operations() {
        let keypair_result = hpke_generate_keypair();
        assert!(keypair_result.is_ok());

        let encrypt_result = hpke_encrypt("test_key", "test_data", None);
        assert!(encrypt_result.is_ok());
    }
}
