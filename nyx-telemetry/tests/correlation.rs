#![forbid(unsafe_code)]
use nyx_telemetry::{TelemetryCollector, TelemetryConfig, record_stream_send};

// Integration (simplified): direct hook invocation simulates nyx-stream send and updates counter.
#[tokio::test]
async fn stream_send_hook_increments_counter_and_system_metrics_collects() {
    let cfg = TelemetryConfig { metrics_enabled: true, metrics_port: 19091, collection_interval: 60, otlp_enabled: false, otlp_endpoint: None, trace_sampling: 1.0, attribute_filter_config: None, exporter_recovery: true };
    let collector = TelemetryCollector::new(cfg).expect("collector");
    collector.init_light().await.expect("init light");

    let before = collector.stream_send_count(7).unwrap_or(0);
    record_stream_send(7, "cid-test");
    record_stream_send(7, "cid-test");
    let after = collector.stream_send_count(7).unwrap_or(0);
    assert!(after >= before + 2, "counter didn't increase as expected: before={} after={}", before, after);

    // Force system metrics collection and assert gauges updated (non-negative & plausible bounds)
    collector.collect_once_for_test().await;
    let (cpu, mem) = collector.current_cpu_mem();
    assert!(cpu >= 0.0 && cpu <= 100.0, "cpu out of range: {}", cpu);
    assert!(mem >= 0.0 && mem <= 100.0, "mem out of range: {}", mem);
}
