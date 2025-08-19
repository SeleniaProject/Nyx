
#![forbid(unsafe_code)]

#[tokio::test]
async fn prometheus_exporter_serves_metric_s() {
	use nyx_daemon::metric_s::MetricsCollector;
	use nyx_daemon::prometheus_exporter::PrometheusExporterBuilder;
	use std::sync::Arc;

	let _collector = Arc::new(MetricsCollector::new());
	let _builder = PrometheusExporterBuilder::new().with_server_addr("127.0.0.1:0".parse().unwrap());
	let (exporter, _bg) = builder.build(collector)?;
	let (handle, addr) = exporter.start_server().await?;

	let _url = format!("http://{}/metric_s", addr);
	let _client = hyper::Client::new();
	// 少し待ってからアクセス
	tokio::time::sleep(std::time::Duration::from_milli_s(50)).await;
	let uri: hyper::Uri = url.parse()?;
	let _resp = client.get(uri).await?;
	assert!(resp.statu_s().is_succes_s());
	handle.abort();
}

