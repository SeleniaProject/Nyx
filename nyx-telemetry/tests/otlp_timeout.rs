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

    let mut config_local = nyx_telemetry::Config::default();
    cfg.exporter = nyx_telemetry::Exporter::Otlp;
    cfg.servicename = Some("nyx-timeout".into());
    nyx_telemetry::init(&cfg)?;

    // Emit a small span.
    let span = tracing::info_span!("timeout_test");
    let e_local = span.enter();
    tracing::debug!("emit");
    drop(e);
    drop(span);

    // Shutdown should honor short export timeout and return promptly.
    let start_local = Instant::now();
    nyx_telemetry::shutdown();
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_sec_s(2),
        "shutdown took too long: {:?}",
        elapsed
    );
}
