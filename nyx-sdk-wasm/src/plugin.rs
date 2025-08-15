//! WASM-side Plugin registry facade and manifest validation.
//!
//! This module offers a secure, minimal plugin registry for browser clients.
//! It validates plugin manifests and signatures client-side before advertising
//! requirements to a Nyx peer. This is a WASM-safe subset that does not
//! perform dynamic code loading; it only manages metadata and permissions.

use base64::Engine;
use ciborium::ser::into_writer as cbor_serialize;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

/// Known permission strings for WASM environment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    ReceiveFrames,
    Handshake,
    DataAccess,
    Control,
    ErrorReporting,
    MetricsAccess,
    CryptoAccess,
}

/// Plugin manifest structure advertised over Nyx protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: u32,
    pub name: String,
    pub version: String,
    pub description: String,
    pub permissions: Vec<Permission>,
    pub required: bool,
    /// Base64 signature of canonical JSON below
    pub signature_b64: Option<String>,
    /// Base64 public key for validation, if not out-of-band
    pub registry_pubkey_b64: Option<String>,
}

impl PluginManifest {
    fn canonical_json(&self) -> String {
        // Canonicalize as a minimal JSON map in a stable field order
        #[derive(Serialize)]
        struct Canonical<'a> {
            id: u32,
            name: &'a str,
            version: &'a str,
            description: &'a str,
            permissions: &'a [Permission],
            required: bool,
        }
        let c = Canonical {
            id: self.id,
            name: &self.name,
            version: &self.version,
            description: &self.description,
            permissions: &self.permissions,
            required: self.required,
        };
        serde_json::to_string(&c).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub ok: bool,
    pub message: String,
}

#[wasm_bindgen]
pub struct PluginRegistryWasm {
    manifests: Vec<PluginManifest>,
}

#[wasm_bindgen]
impl PluginRegistryWasm {
    #[wasm_bindgen(constructor)]
    pub fn new() -> PluginRegistryWasm {
        PluginRegistryWasm {
            manifests: Vec::new(),
        }
    }

    /// Add a plugin manifest (JSON string). Performs structural validation.
    pub fn add_manifest(&mut self, manifest_json: String) -> Result<(), JsValue> {
        let manifest: PluginManifest = serde_json::from_str(&manifest_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid manifest JSON: {}", e)))?;
        // Basic checks
        if manifest.name.trim().is_empty() {
            return Err(JsValue::from_str("Manifest name empty"));
        }
        if Version::parse(&manifest.version).is_err() {
            return Err(JsValue::from_str("Manifest version not semver"));
        }
        self.manifests.push(manifest);
        Ok(())
    }

    /// Verify all manifests that include signatures; returns JSON array of results.
    pub fn verify_all(&self) -> Result<JsValue, JsValue> {
        let results: Vec<ValidationResult> =
            self.manifests.iter().map(|m| verify_manifest(m)).collect();
        serde_wasm_bindgen::to_value(&results).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Export manifests as JSON array string (for advertising during handshake)
    pub fn export_manifests_json(&self) -> String {
        serde_json::to_string(&self.manifests).unwrap_or_else(|_| "[]".to_string())
    }

    /// Export required plugin IDs as CBOR bytes (URL-safe base64)
    /// This matches nyx-stream SETTINGS::PLUGIN_REQUIRED encoding.
    pub fn export_required_plugins_cbor_b64(&self) -> Result<String, JsValue> {
        let required_ids: Vec<u32> = self
            .manifests
            .iter()
            .filter(|m| m.required)
            .map(|m| m.id)
            .collect();
        let mut buf = Vec::new();
        cbor_serialize(&required_ids, &mut buf)
            .map_err(|e| JsValue::from_str(&format!("CBOR encode error: {:?}", e)))?;
        Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf))
    }

    /// Import multiple manifests from JSON array string
    pub fn import_manifests_json(&mut self, manifests_json: String) -> Result<(), JsValue> {
        let list: Vec<PluginManifest> = serde_json::from_str(&manifests_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid manifests JSON: {}", e)))?;
        for m in list {
            self.manifests.push(m);
        }
        Ok(())
    }

    /// Mark a plugin as required or optional by id
    pub fn set_required(&mut self, id: u32, required: bool) {
        for m in &mut self.manifests {
            if m.id == id {
                m.required = required;
            }
        }
    }

    /// Export plugin IDs (required or optional) as JSON array
    pub fn export_plugin_ids_json(&self, required_only: bool) -> String {
        let ids: Vec<u32> = self
            .manifests
            .iter()
            .filter(|m| !required_only || m.required)
            .map(|m| m.id)
            .collect();
        serde_json::to_string(&ids).unwrap_or_else(|_| "[]".to_string())
    }
}

fn verify_manifest(m: &PluginManifest) -> ValidationResult {
    // If no signature present, mark as passed with a note.
    let Some(sig_b64) = &m.signature_b64 else {
        return ValidationResult {
            ok: true,
            message: "No signature attached".to_string(),
        };
    };
    let Some(pk_b64) = &m.registry_pubkey_b64 else {
        return ValidationResult {
            ok: false,
            message: "Missing registry public key".to_string(),
        };
    };
    let Ok(sig_bytes) = base64::engine::general_purpose::STANDARD.decode(sig_b64) else {
        return ValidationResult {
            ok: false,
            message: "Invalid base64 signature".to_string(),
        };
    };
    let Ok(pk_bytes) = base64::engine::general_purpose::STANDARD.decode(pk_b64) else {
        return ValidationResult {
            ok: false,
            message: "Invalid base64 public key".to_string(),
        };
    };
    let Ok(pk_arr) = <[u8; 32]>::try_from(pk_bytes.as_slice()) else {
        return ValidationResult {
            ok: false,
            message: "Invalid public key length".to_string(),
        };
    };
    let Ok(vk) = VerifyingKey::from_bytes(&pk_arr) else {
        return ValidationResult {
            ok: false,
            message: "Invalid public key length".to_string(),
        };
    };
    let Ok(sig_arr) = <[u8; 64]>::try_from(sig_bytes.as_slice()) else {
        return ValidationResult {
            ok: false,
            message: "Invalid signature length".to_string(),
        };
    };
    let sig = Signature::from_bytes(&sig_arr);
    let msg = m.canonical_json();
    let digest = Sha256::digest(msg.as_bytes());
    match vk.verify(digest.as_slice(), &sig) {
        Ok(_) => ValidationResult {
            ok: true,
            message: "Signature valid".to_string(),
        },
        Err(_) => ValidationResult {
            ok: false,
            message: "Signature verification failed".to_string(),
        },
    }
}
