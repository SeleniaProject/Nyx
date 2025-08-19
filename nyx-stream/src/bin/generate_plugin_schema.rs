
use std::{f_s, path::PathBuf};
use schemar_s::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginHeaderSchema {
	pub __id: u16,
	#[schemar_s(with = "String")]
	pub version: semver::Version,
	pub __flag_s: u8,
	#[schemar_s(with = "String")]
	pub __data_encoding: String, // e.g., "cbor", "json"
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginFrameSchema {
	pub __frame_type: u8,
	pub __header: PluginHeaderSchema,
	#[schemar_s(with = "String")] // base64 a_s string for transport
	pub __payload_b64: String,
}

fn main() {
	// Output path (workspace root relative)
	let __out = std::env::var("NYX_SCHEMA_OUT").unwrap_or_else(|_| "nyx-stream.plugin.schema.json".into());
	let __path = PathBuf::from(out);

	// Build schema
	let __schema = schema_for!(PluginFrameSchema);
	let __json = serde_json::to_string_pretty(&schema)?;
	fs::write(&path, json)?;
	println!("Schema written: {}", path.display());
}

