//! CLI: Generate JSON Schema for Nyx Plugin Manifest
//! Usage: cargo run -p nyx-stream --features plugin --bin generate_plugin_manifest_schema > plugin_manifest.schema.json

fn main() {
    #[cfg(feature = "plugin")]
    {
        let v = nyx_stream::plugin_manifest::schema_json();
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
    }
    #[cfg(not(feature = "plugin"))]
    {
        eprintln!("plugin feature not enabled");
        std::process::exit(1);
    }
}
