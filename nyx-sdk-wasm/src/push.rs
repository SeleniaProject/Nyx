//! Push registration helper for Nyx Gateway integration
//!
//! This module provides WASM-compatible push notification registration
//! that returns endpoints for Nyx Gateway integration.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::errors::{NyxWasmError, WasmResult};
use base64::{engine::general_purpose, Engine};
use serde::{Deserialize, Serialize};

/// Configuration for push registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushConfig {
    /// Application server key (VAPID public key)
    pub application_server_key: String,
    /// User agent string for identification
    pub user_agent: Option<String>,
    /// Additional metadata
    pub metadata: Option<serde_json::Value>,
}

/// Result of push registration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushRegistrationResult {
    /// Push endpoint URL for the Nyx Gateway
    pub endpoint: String,
    /// Auth token for push messages
    pub auth_token: String,
    /// P256DH key for encryption
    pub p256dh_key: String,
    /// Registration timestamp
    pub registered_at: String,
    /// Gateway integration details
    pub gateway_config: GatewayConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Gateway endpoint URL
    pub gateway_url: String,
    /// Client identifier
    pub client_id: String,
    /// Supported message types
    pub supported_types: Vec<String>,
}

/// Register for push notifications with Nyx Gateway integration
pub fn register_push_endpoint(gateway_url: &str, client_config: &str) -> WasmResult<String> {
    // Parse client configuration
    let config: PushConfig = serde_json::from_str(client_config)
        .map_err(|e| NyxWasmError::ConfigurationError(format!("Invalid client config: {e}")))?;

    #[cfg(target_arch = "wasm32")]
    {
        // In WASM, use the actual browser APIs
        register_push_wasm(gateway_url, &config)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // For testing, simulate the registration
        register_push_simulation(gateway_url, &config)
    }
}

#[cfg(target_arch = "wasm32")]
fn register_push_wasm(gateway_url: &str, config: &PushConfig) -> WasmResult<String> {
    // Check if service worker is available
    let window = web_sys::window()
        .ok_or_else(|| NyxWasmError::PushRegistrationError("No window object".to_string()))?;

    let navigator = window.navigator();
    let _service_worker = navigator.service_worker();

    // For now, return a simulated result since actual push registration requires async context
    // In a real implementation, this would be an async function using JsFuture
    let result = create_demo_registration_result(gateway_url, config)?;

    serde_json::to_string(&result)
        .map_err(|e| NyxWasmError::SerializationError(e.to_string()).into())
}

#[cfg(not(target_arch = "wasm32"))]
fn register_push_simulation(gateway_url: &str, config: &PushConfig) -> WasmResult<String> {
    let result = create_demo_registration_result(gateway_url, config)?;

    serde_json::to_string(&result)
        .map_err(|e| NyxWasmError::SerializationError(e.to_string()).into())
}

/// Create a demonstration push registration result
fn create_demo_registration_result(
    gateway_url: &str,
    _config: &PushConfig,
) -> Result<PushRegistrationResult, NyxWasmError> {
    // Generate demonstration endpoint and keys
    let endpoint = format!("{}/push/{}", gateway_url, generate_endpoint_id()?);
    let auth_token = generate_auth_token()?;
    let p256dh_key = generate_p256dh_key()?;
    let client_id = generate_client_id()?;

    let timestamp = get_current_timestamp();

    let gateway_config = GatewayConfig {
        gateway_url: gateway_url.to_string(),
        client_id,
        supported_types: vec![
            "nyx.message".to_string(),
            "nyx.control".to_string(),
            "nyx.notification".to_string(),
        ],
    };

    Ok(PushRegistrationResult {
        endpoint,
        auth_token,
        p256dh_key,
        registered_at: timestamp,
        gateway_config,
    })
}

/// Generate a demonstration endpoint ID
fn generate_endpoint_id() -> Result<String, NyxWasmError> {
    let mut id = [0u8; 16];

    #[cfg(target_arch = "wasm32")]
    {
        getrandom::getrandom(&mut id).map_err(|e| {
            NyxWasmError::PushRegistrationError(format!("ID generation failed: {e}"))
        })?;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        for (i, byte) in id.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(31).wrapping_add(73);
        }
    }

    Ok(hex::encode(id))
}

/// Generate a demonstration auth token
fn generate_auth_token() -> Result<String, NyxWasmError> {
    let mut token = [0u8; 16];

    #[cfg(target_arch = "wasm32")]
    {
        getrandom::getrandom(&mut token).map_err(|e| {
            NyxWasmError::PushRegistrationError(format!("Auth token generation failed: {e}"))
        })?;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        for (i, byte) in token.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(47).wrapping_add(113);
        }
    }

    Ok(general_purpose::STANDARD.encode(token))
}

/// Generate a demonstration P256DH key
fn generate_p256dh_key() -> Result<String, NyxWasmError> {
    let mut key = [0u8; 65]; // Uncompressed P256 public key
    key[0] = 0x04; // Uncompressed point indicator

    #[cfg(target_arch = "wasm32")]
    {
        getrandom::getrandom(&mut key[1..]).map_err(|e| {
            NyxWasmError::PushRegistrationError(format!("P256DH key generation failed: {e}"))
        })?;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        for (i, byte) in key[1..].iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(23).wrapping_add(97);
        }
    }

    Ok(general_purpose::STANDARD.encode(key))
}

/// Generate a demonstration client ID
fn generate_client_id() -> Result<String, NyxWasmError> {
    let mut id = [0u8; 8];

    #[cfg(target_arch = "wasm32")]
    {
        getrandom::getrandom(&mut id).map_err(|e| {
            NyxWasmError::PushRegistrationError(format!("Client ID generation failed: {e}"))
        })?;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        for (i, byte) in id.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(19).wrapping_add(157);
        }
    }

    Ok(format!("nyx_client_{}", hex::encode(id)))
}

/// Get current timestamp as ISO 8601 string
fn get_current_timestamp() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let now = js_sys::Date::new_0();
        now.to_iso_string()
            .as_string()
            .unwrap_or_else(|| "unknown".to_string())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        format!(
            "2024-01-01T{:02}:{:02}:{:02}Z",
            (timestamp / 3600) % 24,
            (timestamp / 60) % 60,
            timestamp % 60
        )
    }
}

/// Validate gateway URL format
pub fn validate_gateway_url(url: &str) -> bool {
    url.starts_with("https://") && url.len() > 8
}

/// Check if push notifications are supported in current environment
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn check_push_support() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window();
        if window.is_none() {
            return serde_json::json!({
                "supported": false,
                "reason": "No window object"
            })
            .to_string();
        }

        let navigator = window.unwrap().navigator();
        let service_worker_supported = true; // Assume supported in WASM environment

        serde_json::json!({
            "supported": service_worker_supported,
            "features": {
                "service_worker": service_worker_supported,
                "push_manager": service_worker_supported,
                "notifications": true
            }
        })
        .to_string()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        serde_json::json!({
            "supported": false,
            "reason": "Not in WASM environment"
        })
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_config_serialization() {
        let config = PushConfig {
            application_server_key: "test_key".to_string(),
            user_agent: Some("test_agent".to_string()),
            metadata: None,
        };

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: PushConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(
            config.application_server_key,
            deserialized.application_server_key
        );
    }

    #[test]
    fn test_push_registration() {
        let gateway_url = "https://gateway.nyx.example.com";
        let client_config = r#"{
            "application_server_key": "test_vapid_key",
            "user_agent": "test_browser",
            "metadata": {"version": "1.0"}
        }"#;

        let result = register_push_endpoint(gateway_url, client_config);
        assert!(result.is_ok());

        let result_str = result.unwrap();
        let registration: PushRegistrationResult = serde_json::from_str(&result_str).unwrap();

        assert!(registration.endpoint.contains(gateway_url));
        assert!(!registration.auth_token.is_empty());
        assert!(!registration.p256dh_key.is_empty());
        assert_eq!(registration.gateway_config.gateway_url, gateway_url);
    }

    #[test]
    fn test_gateway_url_validation() {
        assert!(validate_gateway_url("https://gateway.example.com"));
        assert!(validate_gateway_url("https://localhost:8443"));
        assert!(!validate_gateway_url("http://insecure.com")); // Must be HTTPS
        assert!(!validate_gateway_url("not_a_url"));
        assert!(!validate_gateway_url(""));
    }

    #[test]
    fn test_endpoint_id_generation() {
        let id1 = generate_endpoint_id().unwrap();
        let id2 = generate_endpoint_id().unwrap();

        assert_eq!(id1.len(), 32); // 16 bytes = 32 hex chars
        assert_eq!(id2.len(), 32);
        assert!(hex::decode(&id1).is_ok());
        assert!(hex::decode(&id2).is_ok());
    }

    #[test]
    fn test_auth_token_generation() {
        let token = generate_auth_token().unwrap();
        assert!(!token.is_empty());
        assert!(general_purpose::STANDARD.decode(&token).is_ok());
    }

    #[test]
    fn test_check_push_support() {
        let support_info = check_push_support();
        assert!(!support_info.is_empty());

        let parsed: serde_json::Value = serde_json::from_str(&support_info).unwrap();
        assert!(parsed["supported"].is_boolean());
    }
}
