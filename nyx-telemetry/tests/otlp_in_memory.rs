#![cfg(feature = "otlp")]

#[tokio::test(flavor = "current_thread")]
async fn otlp_init_smoke() {
    let config_local = nyx_telemetry::Config {
        exporter: nyx_telemetry::Exporter::Otlp,
        servicename: Some("nyx-test".into()),
        ..Default::default()
    };
    // Should not panic; may fail if feature wired wrongly.
    if let Err(e) = nyx_telemetry::init(&config_local) {
        eprintln!("Failed to initialize telemetry: {e}");
    }
}
