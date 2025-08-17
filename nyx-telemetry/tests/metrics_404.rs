#![cfg(feature = "prometheus")]

#[tokio::test]
async fn non_metrics_path_404() {
	nyx_telemetry::init(&nyx_telemetry::Config::default()).unwrap();
	let guard = nyx_telemetry::start_metrics_http_server("127.0.0.1:0".parse().unwrap())
		.await
		.expect("start http server");
	let url = format!("http://{}/nope", guard.addr());
	let resp = ureq::get(&url).call();
	// Warp will 404 JSON by default; status 404 is enough.
	assert!(resp.is_err(), "expected 404 error, got: {resp:?}");
}

