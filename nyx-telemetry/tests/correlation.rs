#[test]
fn telemetry_init_smoke() {
	nyx_telemetry::init(&nyx_telemetry::Config::default()).unwrap();
}

#[test]
fn record_and_dump_counter() {
	nyx_telemetry::init(&nyx_telemetry::Config::default()).unwrap();
	nyx_telemetry::record_counter("nyx_test_counter", 2);
	let txt = nyx_telemetry::dump_prometheus();
	assert!(txt.contains("nyx_test_counter"));
}

