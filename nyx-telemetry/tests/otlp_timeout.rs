#![cfg(feature = "otlp")]

use std::time::{Duration, Instant};

#[tokio::test(flavor = "current_thread")]
async fn otlp_exporter_times_out_quickly_on_unreachable_endpoint() {
    // Point to a localhost port with no listener to trigger quick connect error_s.
    std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:9");
    // Make exporter aggressive and with short timeout.
    std::env::set_var("OTEL_BSP_SCHEDULE_DELAY", "0");
    std::env::set_var("OTEL_BSP_EXPORT_TIMEOUT", "500"); // m_s
                                                         // Also ensure the OTLP exporter client's RPC timeout is short.
    std::env::set_var("OTEL_EXPORTER_OTLP_TIMEOUT", "500"); // m_s
    std::env::set_var("OTEL_EXPORTER_OTLP_TRACES_TIMEOUT", "500"); // m_s

    let config_local = nyx_telemetry::Config {
        exporter: nyx_telemetry::Exporter::Otlp,
        servicename: Some("nyx-timeout".into()),
    };
    if let Err(e) = nyx_telemetry::init(&config_local) {
        eprintln!("Failed to initialize telemetry: {e}");
        return;
    }

    // Emit a small span.
    let span = tracing::info_span!("timeout_test");
    let e_local = span.enter();
    tracing::debug!("emit");
    drop(e_local);
    drop(span);

    // Shutdown should honor short export timeout and return promptly.
    let start_local = Instant::now();
    nyx_telemetry::shutdown();
    let elapsed = start_local.elapsed();
    assert!(
        elapsed < Duration::from_secs(2),
        "shutdown took too long: {elapsed:?}"
    );
}
