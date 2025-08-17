
use std::{fs, path::PathBuf};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginHeaderSchema {
	pub id: u16,
	#[schemars(with = "String")]
	pub version: semver::Version,
	pub flags: u8,
	#[schemars(with = "String")]
	pub data_encoding: String, // e.g., "cbor", "json"
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginFrameSchema {
	pub frame_type: u8,
	pub header: PluginHeaderSchema,
	#[schemars(with = "String")] // base64 as string for transport
	pub payload_b64: String,
}

fn main() {
	// Output path (workspace root relative)
	let out = std::env::var("NYX_SCHEMA_OUT").unwrap_or_else(|_| "nyx-stream.plugin.schema.json".into());
	let path = PathBuf::from(out);

	// Build schema
	let schema = schema_for!(PluginFrameSchema);
	let json = serde_json::to_string_pretty(&schema).expect("serialize schema");
	fs::write(&path, json).expect("write schema json");
	println!("Schema written: {}", path.display());
}

