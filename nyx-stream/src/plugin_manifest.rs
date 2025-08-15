#![forbid(unsafe_code)]

//! Plugin Manifest utilities: JSON Schema generation and strict validation
//!
//! This module defines the manifest format used to authorize plugins via
//! Ed25519 signatures and version/capability constraints. It provides
//! functions to export the JSON Schema and to validate/parse manifest files.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One entry in the plugin manifest
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ManifestItem {
    /// Unique numeric plugin identifier
    pub id: u32,
    /// Minimum supported version (major, minor)
    pub min_version: (u16, u16),
    /// Maximum supported version (major, minor)
    pub max_version: (u16, u16),
    /// Base64-encoded Ed25519 public key (32 bytes)
    pub pubkey_b64: String,
    /// Base64-encoded Ed25519 signature (64 bytes) over canonical message
    /// "plugin:{id}:v1"
    pub signature_b64: String,
    /// Capability strings authorized for this plugin
    #[serde(default)]
    pub caps: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum ManifestError {
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),

    #[error("Schema validation failed: {0}")]
    SchemaValidation(String),
}

/// Export JSON Schema for the manifest format (an array of items)
pub fn schema_json() -> serde_json::Value {
    let schema = schemars::schema_for!(Vec<ManifestItem>);
    serde_json::to_value(&schema).expect("schema to json")
}

/// Validate a manifest JSON string against the schema and parse into items
pub fn validate_and_parse(json_str: &str) -> Result<Vec<ManifestItem>, ManifestError> {
    let value: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| ManifestError::InvalidJson(e.to_string()))?;

    let compiled = jsonschema::JSONSchema::compile(&schema_json())
        .map_err(|e| ManifestError::SchemaValidation(e.to_string()))?;

    if let Err(errors) = compiled.validate(&value) {
        let mut buf = String::new();
        for (i, err) in errors.enumerate() {
            if i > 0 {
                buf.push_str("; ");
            }
            buf.push_str(&err.to_string());
        }
        return Err(ManifestError::SchemaValidation(buf));
    }

    serde_json::from_value::<Vec<ManifestItem>>(value)
        .map_err(|e| ManifestError::InvalidJson(e.to_string()))
}

/// Convenience: read and validate a manifest file path
pub fn read_and_parse_file<P: AsRef<std::path::Path>>(
    path: P,
) -> Result<Vec<ManifestItem>, ManifestError> {
    let data =
        std::fs::read_to_string(path).map_err(|e| ManifestError::InvalidJson(e.to_string()))?;
    validate_and_parse(&data)
}
