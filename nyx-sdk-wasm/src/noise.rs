//! Noise handshake demonstration for WASM environment
//!
//! This module provides a cryptographic showcase of the Noise protocol
//! in a browser-compatible WASM environment.

use crate::errors::{NyxWasmError, WasmResult};
use serde::{Deserialize, Serialize};

/// Configuration for Noise handshake demonstration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseConfig {
    /// Protocol pattern (e.g., "Noise_XX_25519_ChaChaPoly_BLAKE2s")
    pub pattern: String,
    /// Pre-shared key (optional, hex-encoded)
    pub psk: Option<String>,
    /// Static key pair (optional, hex-encoded)
    pub static_keypair: Option<StaticKeypair>,
    /// Additional data for handshake
    pub payload: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticKeypair {
    pub private_key: String,
    pub public_key: String,
}

/// Result of a Noise handshake demonstration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResult {
    /// Whether handshake completed successfully
    pub success: bool,
    /// Shared secret (hex-encoded)
    pub shared_secret: Option<String>,
    /// Initiator's ephemeral public key
    pub initiator_ephemeral: String,
    /// Responder's ephemeral public key  
    pub responder_ephemeral: String,
    /// Handshake messages exchanged
    pub messages: Vec<HandshakeMessage>,
    /// Performance metrics
    pub metrics: HandshakeMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeMessage {
    pub direction: String, // "initiator->responder" or "responder->initiator"
    pub payload_size: usize,
    pub message_type: String, // "e", "es", "s", "ss" etc.
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeMetrics {
    pub total_time_ms: f64,
    pub key_generation_time_ms: f64,
    pub exchange_time_ms: f64,
    pub total_bytes: usize,
}

/// Perform a complete Noise handshake demonstration
pub fn perform_handshake_demo(
    initiator_config: &str,
    responder_config: &str,
) -> WasmResult<String> {
    let start_time = get_timestamp();

    // Parse configurations
    let initiator_cfg: NoiseConfig = serde_json::from_str(initiator_config)
        .map_err(|e| NyxWasmError::ConfigurationError(format!("Invalid initiator config: {e}")))?;

    let responder_cfg: NoiseConfig = serde_json::from_str(responder_config)
        .map_err(|e| NyxWasmError::ConfigurationError(format!("Invalid responder config: {e}")))?;

    // Validate protocol compatibility
    if initiator_cfg.pattern != responder_cfg.pattern {
        return Err(NyxWasmError::HandshakeError(
            "Protocol patterns must match between initiator and responder".to_string(),
        )
        .into());
    }

    // Generate demonstration handshake
    let keygen_start = get_timestamp();
    let handshake_result = simulate_noise_handshake(&initiator_cfg, &responder_cfg)?;
    let keygen_time = get_timestamp() - keygen_start;

    let exchange_start = get_timestamp();
    // Simulate message exchange timing
    #[cfg(target_arch = "wasm32")]
    {
        // Add small delay to simulate network latency
        let _ = js_sys::Date::now();
    }
    let exchange_time = get_timestamp() - exchange_start;

    let total_time = get_timestamp() - start_time;

    let mut result = handshake_result;
    result.metrics = HandshakeMetrics {
        total_time_ms: total_time,
        key_generation_time_ms: keygen_time,
        exchange_time_ms: exchange_time,
        total_bytes: result.messages.iter().map(|m| m.payload_size).sum(),
    };

    serde_json::to_string(&result)
        .map_err(|e| NyxWasmError::SerializationError(e.to_string()).into())
}

/// Simulate a Noise handshake for demonstration purposes
fn simulate_noise_handshake(
    initiator_cfg: &NoiseConfig,
    _responder_cfg: &NoiseConfig,
) -> Result<HandshakeResult, NyxWasmError> {
    // Generate ephemeral keypairs (simulated with random data)
    let initiator_ephemeral = generate_demo_key()?;
    let responder_ephemeral = generate_demo_key()?;

    let mut messages = Vec::new();

    // Simulate XX pattern handshake messages
    match initiator_cfg.pattern.as_str() {
        "Noise_XX_25519_ChaChaPoly_BLAKE2s" => {
            // Message 1: e
            messages.push(HandshakeMessage {
                direction: "initiator->responder".to_string(),
                payload_size: 32 + 16, // ephemeral key + auth tag
                message_type: "e".to_string(),
            });

            // Message 2: e, ee, s, es
            messages.push(HandshakeMessage {
                direction: "responder->initiator".to_string(),
                payload_size: 32 + 48 + 16, // ephemeral + encrypted static + auth tag
                message_type: "e,ee,s,es".to_string(),
            });

            // Message 3: s, se
            messages.push(HandshakeMessage {
                direction: "initiator->responder".to_string(),
                payload_size: 48 + 16, // encrypted static + auth tag
                message_type: "s,se".to_string(),
            });
        }
        _ => {
            return Err(NyxWasmError::HandshakeError(format!(
                "Unsupported pattern: {}",
                initiator_cfg.pattern
            )));
        }
    }

    // Generate demonstration shared secret
    let shared_secret = generate_demo_secret()?;

    Ok(HandshakeResult {
        success: true,
        shared_secret: Some(shared_secret),
        initiator_ephemeral,
        responder_ephemeral,
        messages,
        metrics: HandshakeMetrics {
            total_time_ms: 0.0,
            key_generation_time_ms: 0.0,
            exchange_time_ms: 0.0,
            total_bytes: 0,
        },
    })
}

/// Generate a demonstration key (32 random bytes, hex-encoded)
fn generate_demo_key() -> Result<String, NyxWasmError> {
    let mut key = [0u8; 32];

    #[cfg(target_arch = "wasm32")]
    {
        getrandom::getrandom(&mut key)
            .map_err(|e| NyxWasmError::CryptographicError(format!("Key generation failed: {e}")))?;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // For non-WASM testing, use a deterministic pattern
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(17).wrapping_add(42);
        }
    }

    Ok(hex::encode(key))
}

/// Generate a demonstration shared secret
fn generate_demo_secret() -> Result<String, NyxWasmError> {
    generate_demo_key()
}

/// Get current timestamp in milliseconds
fn get_timestamp() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64
    }
}

/// Validate Noise protocol pattern
pub fn validate_pattern(pattern: &str) -> bool {
    matches!(
        pattern,
        "Noise_XX_25519_ChaChaPoly_BLAKE2s"
            | "Noise_IK_25519_ChaChaPoly_BLAKE2s"
            | "Noise_NK_25519_ChaChaPoly_BLAKE2s"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_config_serialization() {
        let config = NoiseConfig {
            pattern: "Noise_XX_25519_ChaChaPoly_BLAKE2s".to_string(),
            psk: None,
            static_keypair: None,
            payload: Some("test payload".to_string()),
        };

        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: NoiseConfig = serde_json::from_str(&serialized).unwrap();

        assert_eq!(config.pattern, deserialized.pattern);
    }

    #[test]
    fn test_handshake_demo() {
        let initiator_config = r#"{
            "pattern": "Noise_XX_25519_ChaChaPoly_BLAKE2s",
            "psk": null,
            "static_keypair": null,
            "payload": "initiator_data"
        }"#;

        let responder_config = r#"{
            "pattern": "Noise_XX_25519_ChaChaPoly_BLAKE2s",
            "psk": null,
            "static_keypair": null,
            "payload": "responder_data"
        }"#;

        let result = perform_handshake_demo(initiator_config, responder_config);
        assert!(result.is_ok());

        let result_str = result.unwrap();
        let handshake_result: HandshakeResult = serde_json::from_str(&result_str).unwrap();

        assert!(handshake_result.success);
        assert!(handshake_result.shared_secret.is_some());
        assert_eq!(handshake_result.messages.len(), 3); // XX pattern has 3 messages
    }

    #[test]
    fn test_pattern_validation() {
        assert!(validate_pattern("Noise_XX_25519_ChaChaPoly_BLAKE2s"));
        assert!(validate_pattern("Noise_IK_25519_ChaChaPoly_BLAKE2s"));
        assert!(!validate_pattern("Invalid_Pattern"));
    }

    #[test]
    fn test_key_generation() {
        let key1 = generate_demo_key().unwrap();
        let key2 = generate_demo_key().unwrap();

        assert_eq!(key1.len(), 64); // 32 bytes = 64 hex chars
        assert_eq!(key2.len(), 64);

        // Keys should be valid hex
        assert!(hex::decode(&key1).is_ok());
        assert!(hex::decode(&key2).is_ok());
    }
}
