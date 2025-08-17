
use std::{fs, path::PathBuf};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PermissionSpec {
	pub name: String,
	#[schemars(default)]
	pub required: bool,
	#[schemars(default)]
	pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginManifestSchema {
	pub plugin_id: u16,
	pub name: String,
	#[schemars(with = "String")] // SemVer
	pub version: semver::Version,
	#[schemars(default)]
	pub description: Option<String>,
	#[schemars(default)]
	pub permissions: Vec<PermissionSpec>,
	#[schemars(default)]
	pub compatible_core: Option<String>, // e.g., ">=1.0.0 <2.0.0"
}

fn main() {
	let out = std::env::var("NYX_MANIFEST_SCHEMA_OUT").unwrap_or_else(|_| "nyx-stream.plugin-manifest.schema.json".into());
	let path = PathBuf::from(out);
	let schema = schema_for!(PluginManifestSchema);
	let json = serde_json::to_string_pretty(&schema).expect("serialize schema");
	fs::write(&path, json).expect("write schema json");
	println!("Manifest schema written: {}", path.display());
}

