#![cfg(feature = "otlp")]
// Ensures that emitting a nyx.stream.send span increments the stream send counter via span hook.
use nyx_telemetry::{TelemetryCollector, TelemetryConfig};
#[tokio::test]
async fn span_increments_stream_send_counter() {
    let cfg = TelemetryConfig {
        metrics_enabled: true,
        metrics_port: 19444,
        collection_interval: 60,
        otlp_enabled: true,
        otlp_endpoint: None,
        trace_sampling: 1.0,
        attribute_filter_config: None,
        exporter_recovery: false,
    };
    let collector = TelemetryCollector::new(cfg).unwrap();
    collector.init_light().await.unwrap();
    let before = collector.stream_send_count(7).unwrap_or(0);
    {
        let span = tracing::span!(
            tracing::Level::INFO,
            "nyx.stream.send",
            path_id = 7u8,
            cid = "cid-span-metric"
        );
        let _e = span.enter();
    }
    // drop triggers on_close; hook increments counter
    let after = collector.stream_send_count(7).unwrap_or(0);
    assert_eq!(
        after,
        before + 1,
        "stream send counter did not increment via span hook before={} after={}",
        before,
        after
    );
}
