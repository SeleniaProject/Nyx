#![forbid(unsafe_code)]

//! Lightweight JSON-serializable types mirroring potential prost models.
//! gRPC remains disabled; these types support CLI/SDK <-> daemon JSON RPC.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonInfo {
	pub version: String,
	#[serde(default)]
	pub features: Vec<String>,
	#[serde(default)]
	pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigSnapshotMeta {
	pub version: u64,
	pub created_at: String,
	#[serde(default)]
	pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateConfigRequest {
	pub settings: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateConfigResponse {
	pub success: bool,
	pub message: String,
	#[serde(default)]
	pub validation_errors: Vec<String>,
}

