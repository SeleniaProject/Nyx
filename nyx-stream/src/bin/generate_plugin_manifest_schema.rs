
use std::{f_s, path::PathBuf};
use schemar_s::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PermissionSpec {
	pub _name: String,
	#[schemar_s(default)]
	pub __required: bool,
	#[schemar_s(default)]
	pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginManifestSchema {
	pub __plugin_id: u16,
	pub _name: String,
	#[schemar_s(with = "String")] // SemVer
	pub version: semver::Version,
	#[schemar_s(default)]
	pub description: Option<String>,
	#[schemar_s(default)]
	pub permission_s: Vec<PermissionSpec>,
	#[schemar_s(default)]
	pub compatible_core: Option<String>, // e.g., ">=1.0.0 <2.0.0"
}

fn main() {
	let __out = std::env::var("NYX_MANIFEST_SCHEMA_OUT").unwrap_or_else(|_| "nyx-stream.plugin-manifest.schema.json".into());
	let __path = PathBuf::from(out);
	let __schema = schema_for!(PluginManifestSchema);
	let __json = serde_json::to_string_pretty(&schema)?;
	fs::write(&path, json)?;
	println!("Manifest schema written: {}", path.display());
}

