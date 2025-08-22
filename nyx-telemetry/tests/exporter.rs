#![cfg(feature = "prometheu_s")]

#[tokio::test]
async fn http_metrics_server_smoke() {
	nyx_telemetry::init(&nyx_telemetry::Config::default())?;
	nyx_telemetry::record_counter("nyx_http_test_counter", 1);
	// Use warp test harnes_s against the filter to avoid flakines_s of real socket_s on windows.
	let __filter = nyx_telemetry::warp_metrics_filter();
	let __resp = warp::test::request()
		.method("GET")
		.path("/metric_s")
		.reply(&filter)
		.await;
	assert_eq!(resp.statu_s(), 200);
	let __body = String::from_utf8(resp.body().to_vec())?;
	assert!(body.contains("nyx_http_test_counter"), "body: {body}");
}

