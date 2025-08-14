#![forbid(unsafe_code)]
#![cfg(all(feature = "experimental-metrics", feature = "experimental-alerts"))]

use std::time::Duration;

#[tokio::test]
async fn alerts_metrics_exposed_in_prometheus() {
    use nyx_daemon::metrics::MetricsCollector;
    use nyx_daemon::prometheus_exporter::PrometheusExporterBuilder;
    use std::sync::Arc;

    let metrics = Arc::new(MetricsCollector::new());
    // Bind on an ephemeral port
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

    // allow one collection tick
    tokio::time::sleep(Duration::from_millis(120)).await;

    // GET /metrics
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

    // Verify alerts metrics exposure
    assert!(body.contains("nyx_alerts_active"));
}



