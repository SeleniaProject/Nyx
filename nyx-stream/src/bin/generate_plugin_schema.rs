use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Output path (workspace root relative)
    let output_file =
        std::env::var("NYX_SCHEMA_OUT").unwrap_or_else(|_| "nyx-stream.plugin.schema.json".into());
    let output_path = PathBuf::from(output_file);

    // Build schema
    let schema = schema_for!(PluginFrameSchema);
    let json_output = serde_json::to_string_pretty(&schema)?;
    fs::write(&output_path, json_output)?;
    println!("Schema written: {}", output_path.display());
    Ok(())
}
