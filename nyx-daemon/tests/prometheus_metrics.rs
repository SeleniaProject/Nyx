
#![forbid(unsafe_code)]

#[tokio::test]
async fn prometheus_exporter_serves_metrics() {
	use nyx_daemon::metrics::MetricsCollector;
	use nyx_daemon::prometheus_exporter::PrometheusExporterBuilder;
	use std::sync::Arc;

	let collector = Arc::new(MetricsCollector::new());
	let builder = PrometheusExporterBuilder::new().with_server_addr("127.0.0.1:0".parse().unwrap());
	let (exporter, _bg) = builder.build(collector).expect("build exporter");
	let (handle, addr) = exporter.start_server().await.expect("start server");

	let url = format!("http://{}/metrics", addr);
	let client = hyper::Client::new();
	// 少し待ってからアクセス
	tokio::time::sleep(std::time::Duration::from_millis(50)).await;
	let uri: hyper::Uri = url.parse().unwrap();
	let resp = client.get(uri).await.expect("http get");
	assert!(resp.status().is_success());
	handle.abort();
}

