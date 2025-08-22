use schemars::schema_for;
use std::{fs, path::PathBuf};
// use schemars::JsonSchema;
// use serde::{Deserialize, Serialize};
use nyx_stream::plugin_manifest::PluginManifest;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::var("NYX_MANIFEST_SCHEMA_OUT")
        .unwrap_or_else(|_| "nyx-stream.plugin-manifest.schema.json".into());
    let path = PathBuf::from(&out);
    let schema = schema_for!(PluginManifest);
    let json = serde_json::to_string_pretty(&schema)?;
    fs::write(&path, &json)?;
    println!("Manifest schema written: {}", path.display());
    Ok(())
}
