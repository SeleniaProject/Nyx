#![forbid(unsafe_code)]
#![cfg(feature = "experimental-metrics")]

use std::time::Duration;

// This test validates Prometheus exporter HTTP endpoint and presence of zero-copy metrics.
#[tokio::test]
async fn prometheus_exposes_metrics_and_zero_copy_labels() {
    use nyx_core::zero_copy::manager::{ZeroCopyManager, ZeroCopyManagerConfig};
    use nyx_daemon::metrics::MetricsCollector;
    use nyx_daemon::prometheus_exporter::PrometheusExporterBuilder;
    use nyx_daemon::zero_copy_bridge::start_zero_copy_metrics_task_with_interval;
    use std::sync::Arc;

    let metrics = Arc::new(MetricsCollector::new());
    // Bind on an ephemeral port to avoid conflicts
    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);

    let exporter = PrometheusExporterBuilder::new()
        .with_server_addr(addr)
        .with_update_interval(Duration::from_millis(50))
        .build(Arc::clone(&metrics))
        .expect("exporter build");

    exporter.start_server().await.expect("start server");
    exporter.start_collection().await.expect("start collection");

    // Kick zero-copy bridge with a short interval so counters appear
    let manager = Arc::new(ZeroCopyManager::new(ZeroCopyManagerConfig::default()));
    start_zero_copy_metrics_task_with_interval(Arc::clone(&manager), Duration::from_millis(50));

    // Allow a few ticks
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Simple manual HTTP/1.1 GET using tokio TCP to avoid extra dev-deps
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /metrics HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        addr
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut resp_buf = Vec::new();
    stream.read_to_end(&mut resp_buf).await.unwrap();
    let resp_txt = String::from_utf8_lossy(&resp_buf);
    assert!(resp_txt.starts_with("HTTP/1.1 200"));
    let body = resp_txt.split("\r\r").last().unwrap_or("").to_string();

    // Basic exporter self-metrics
    assert!(body.contains("nyx_requests_total") || body.contains("nyx_uptime_seconds"));
    // Zero-copy bridge metrics (any of the following should appear)
    assert!(
        body.contains("nyx_zero_copy_combined_allocations")
            || body.contains("nyx_zero_copy_combined_bytes")
            || body.contains("nyx_zero_copy_total_paths"),
        "zero-copy metrics should be present"
    );

    // Process resource gauges presence (best-effort; at least one)
    assert!(
        body.contains("nyx_memory_bytes")
            || body.contains("nyx_open_fds")
            || body.contains("nyx_threads"),
        "process resource gauges should be present"
    );
}
