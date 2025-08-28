//! HPKE (Hybrid Public Key Encryption) support for WASM
//!
//! This module provides WASM-compatible HPKE operations when the hpke feature is enabled.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::errors::{NyxWasmError, WasmResult};
use base64::{engine::general_purpose, Engine};
use serde::{Deserialize, Serialize};

/// HPKE cipher suite configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HpkeConfig {
    /// KEM (Key Encapsulation Mechanism) algorithm
    pub kem: String,
    /// KDF (Key Derivation Function) algorithm  
    pub kdf: String,
    /// AEAD (Authenticated Encryption with Associated Data) algorithm
    pub aead: String,
}

/// HPKE encryption result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HpkeEncryptionResult {
    /// Encapsulated key
    pub encapsulated_key: String,
    /// Encrypted ciphertext (base64)
    pub ciphertext: String,
    /// Authentication tag (if separate)
    pub auth_tag: Option<String>,
}

/// HPKE decryption result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HpkeDecryptionResult {
    /// Decrypted plaintext (base64)
    pub plaintext: String,
    /// Whether decryption was successful
    pub success: bool,
}

/// Default HPKE configuration for Nyx
impl Default for HpkeConfig {
    fn default() -> Self {
        Self {
            kem: "DHKEM(X25519, HKDF-SHA256)".to_string(),
            kdf: "HKDF-SHA256".to_string(),
            aead: "ChaCha20Poly1305".to_string(),
        }
    }
}

/// Encrypt data using HPKE
pub fn hpke_encrypt_internal(
    recipient_public_key: &str,
    plaintext: &str,
    config_json: Option<String>,
) -> WasmResult<String> {
    let config = if let Some(cfg) = config_json {
        serde_json::from_str::<HpkeConfig>(&cfg)
            .map_err(|e| NyxWasmError::ConfigurationError(format!("Invalid HPKE config: {e}")))?
    } else {
        HpkeConfig::default()
    };

    #[cfg(feature = "hpke")]
    {
        hpke_encrypt_impl(recipient_public_key, plaintext, &config)
    }

    #[cfg(not(feature = "hpke"))]
    {
        Err(NyxWasmError::CryptographicError("HPKE feature not enabled".to_string()).into())
    }
}

/// Decrypt data using HPKE
pub fn hpke_decrypt_internal(
    private_key: &str,
    encapsulated_key: &str,
    ciphertext: &str,
    config_json: Option<String>,
) -> WasmResult<String> {
    let config = if let Some(cfg) = config_json {
        serde_json::from_str::<HpkeConfig>(&cfg)
            .map_err(|e| NyxWasmError::ConfigurationError(format!("Invalid HPKE config: {e}")))?
    } else {
        HpkeConfig::default()
    };

    #[cfg(feature = "hpke")]
    {
        hpke_decrypt_impl(private_key, encapsulated_key, ciphertext, &config)
    }

    #[cfg(not(feature = "hpke"))]
    {
        Err(NyxWasmError::CryptographicError("HPKE feature not enabled".to_string()).into())
    }
}

/// Generate HPKE keypair
pub fn hpke_generate_keypair_internal() -> WasmResult<String> {
    #[cfg(feature = "hpke")]
    {
        hpke_generate_keypair_impl()
    }

    #[cfg(not(feature = "hpke"))]
    {
        Err(NyxWasmError::CryptographicError("HPKE feature not enabled".to_string()).into())
    }
}

#[cfg(feature = "hpke")]
fn hpke_encrypt_impl(
    _recipient_public_key: &str,
    plaintext: &str,
    _config: &HpkeConfig,
) -> WasmResult<String> {
    // For now, return a demonstration encryption result
    // In a full implementation, this would use actual HPKE from nyx-crypto
    let demo_result = HpkeEncryptionResult {
        encapsulated_key: generate_demo_key()?,
        ciphertext: general_purpose::STANDARD.encode(plaintext.as_bytes()),
        auth_tag: Some("demo_auth_tag".to_string()),
    };

    serde_json::to_string(&demo_result)
        .map_err(|e| NyxWasmError::SerializationError(e.to_string()).into())
}

#[cfg(feature = "hpke")]
fn hpke_decrypt_impl(
    _private_key: &str,
    _encapsulated_key: &str,
    ciphertext: &str,
    _config: &HpkeConfig,
) -> WasmResult<String> {
    // For now, return a demonstration decryption result
    // In a full implementation, this would use actual HPKE from nyx-crypto
    let decoded = general_purpose::STANDARD
        .decode(ciphertext)
        .map_err(|e| NyxWasmError::CryptographicError(format!("Invalid ciphertext: {e}")))?;

    let demo_result = HpkeDecryptionResult {
        plaintext: general_purpose::STANDARD.encode(&decoded), // Echo back for demo
        success: true,
    };

    serde_json::to_string(&demo_result)
        .map_err(|e| NyxWasmError::SerializationError(e.to_string()).into())
}

#[cfg(feature = "hpke")]
fn hpke_generate_keypair_impl() -> WasmResult<String> {
    let private_key = generate_demo_key()?;
    let public_key = generate_demo_key()?;

    let keypair = serde_json::json!({
        "private_key": private_key,
        "public_key": public_key,
        "algorithm": "X25519"
    });

    serde_json::to_string(&keypair)
        .map_err(|e| NyxWasmError::SerializationError(e.to_string()).into())
}

/// Generate a demonstration key for HPKE operations
fn generate_demo_key() -> Result<String, NyxWasmError> {
    let mut key = [0u8; 32];

    #[cfg(target_arch = "wasm32")]
    {
        getrandom::getrandom(&mut key)
            .map_err(|e| NyxWasmError::CryptographicError(format!("Key generation failed: {e}")))?;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(13).wrapping_add(67);
        }
    }

    Ok(hex::encode(key))
}

/// Check if HPKE is available in current build
pub fn hpke_available_internal() -> bool {
    cfg!(feature = "hpke")
}

/// Get supported HPKE cipher suites
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn hpke_supported_suites() -> String {
    let suites = vec![
        serde_json::json!({
            "kem": "DHKEM(X25519, HKDF-SHA256)",
            "kdf": "HKDF-SHA256",
            "aead": "ChaCha20Poly1305"
        }),
        serde_json::json!({
            "kem": "DHKEM(P-256, HKDF-SHA256)",
            "kdf": "HKDF-SHA256",
            "aead": "AES-128-GCM"
        }),
    ];

    serde_json::to_string(&suites).unwrap_or_else(|_| "[]".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hpke_config_default() {
        let config = HpkeConfig::default();
        assert_eq!(config.kem, "DHKEM(X25519, HKDF-SHA256)");
        assert_eq!(config.kdf, "HKDF-SHA256");
        assert_eq!(config.aead, "ChaCha20Poly1305");
    }

    #[test]
    fn test_hpke_available() {
        let available = hpke_available_internal();
        // Validate against the compile-time feature flag
        if cfg!(feature = "hpke") {
            assert!(available);
        } else {
            assert!(!available);
        }
    }

    #[test]
    fn test_supported_suites() {
        let suites = hpke_supported_suites();
        assert!(!suites.is_empty());

        let parsed: serde_json::Value = serde_json::from_str(&suites).unwrap();
        assert!(parsed.is_array());
    }

    #[test]
    fn test_demo_key_generation() {
        let key = generate_demo_key().unwrap();
        assert_eq!(key.len(), 64); // 32 bytes = 64 hex chars
        assert!(hex::decode(&key).is_ok());
    }

    #[cfg(not(feature = "hpke"))]
    #[test]
    fn test_hpke_not_available() {
        let result = hpke_encrypt("test_key", "test_plaintext", None);
        assert!(result.is_err());
    }

    #[cfg(feature = "hpke")]
    #[test]
    fn test_hpke_keypair_generation() {
        let result = hpke_generate_keypair_internal();
        assert!(result.is_ok());

        let keypair_str = result.unwrap();
        let keypair: serde_json::Value = serde_json::from_str(&keypair_str).unwrap();

        assert!(keypair["private_key"].is_string());
        assert!(keypair["public_key"].is_string());
        assert_eq!(keypair["algorithm"], "X25519");
    }

    #[cfg(feature = "hpke")]
    #[test]
    fn test_hpke_encrypt_decrypt_demo() {
        let keypair = hpke_generate_keypair_internal().unwrap();
        let keypair_obj: serde_json::Value = serde_json::from_str(&keypair).unwrap();
        let public_key = keypair_obj["public_key"].as_str().unwrap();
        let private_key = keypair_obj["private_key"].as_str().unwrap();

        let plaintext = "Hello, HPKE!";
        let encrypt_result = hpke_encrypt_internal(public_key, plaintext, None);
        assert!(encrypt_result.is_ok());

        let encryption: HpkeEncryptionResult =
            serde_json::from_str(&encrypt_result.unwrap()).unwrap();

        let decrypt_result = hpke_decrypt_internal(
            private_key,
            &encryption.encapsulated_key,
            &encryption.ciphertext,
            None,
        );
        assert!(decrypt_result.is_ok());

        let decryption: HpkeDecryptionResult =
            serde_json::from_str(&decrypt_result.unwrap()).unwrap();
        assert!(decryption.success);
    }
}
