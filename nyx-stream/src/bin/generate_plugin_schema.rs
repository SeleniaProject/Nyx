
use std::{fs, path::PathBuf};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginHeaderSchema {
	pub __id: u16,
	#[schemars(with = "String")]
	pub version: semver::Version,
	pub __flag_s: u8,
	#[schemars(with = "String")]
	pub __data_encoding: String, // e.g., "cbor", "json"
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PluginFrameSchema {
	pub __frame_type: u8,
	pub __header: PluginHeaderSchema,
	#[schemars(with = "String")] // base64 as string for transport
	pub __payload_b64: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	// Output path (workspace root relative)
	let __out = std::env::var("NYX_SCHEMA_OUT").unwrap_or_else(|_| "nyx-stream.plugin.schema.json".into());
	let __path = PathBuf::from(__out);

	// Build schema
	let __schema = schema_for!(PluginFrameSchema);
	let __json = serde_json::to_string_pretty(&__schema)?;
	fs::write(&__path, __json)?;
	println!("Schema written: {}", __path.display());
	Ok(())
}

