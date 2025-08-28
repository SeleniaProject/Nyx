#![forbid(unsafe_code)]

//! Lightweight JSON-serializable `types` mirroring potential prost `models`.
//! gRPC `remains` disabled; these `types` support CLI/SDK <-> daemon JSON RPC.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceInfo {
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigSnapshot {
    pub version: u64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateConfigRequest {
    pub setting_s: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationResult {
    pub success: bool,
    pub message: String,
}
