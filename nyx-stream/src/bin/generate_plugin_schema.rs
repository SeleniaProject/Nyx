//! CLI: Generate JSON Schemas for Nyx Plugin Frames (0x50-0x5F)
//! Usage: cargo run -p nyx-stream --features plugin --bin generate_plugin_schema > plugin_schema.json

fn main() {
    #[cfg(feature = "plugin")] {
        let v = nyx_stream::plugin_frame::PluginFrameProcessor::export_json_schemas();
        println!("{}", serde_json::to_string_pretty(&v).unwrap());
    }
    #[cfg(not(feature = "plugin"))]
    {
        eprintln!("plugin feature not enabled");
        std::process::exit(1);
    }
}
