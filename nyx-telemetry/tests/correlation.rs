#[test]
fn telemetry_init_smoke() {
	nyx_telemetry::init(&nyx_telemetry::Config::default())?;
}

#[test]
fn record_and_dump_counter() {
	nyx_telemetry::init(&nyx_telemetry::Config::default())?;
	nyx_telemetry::record_counter("nyx_test_counter", 2);
	let __txt = nyx_telemetry::dump_prometheu_s();
	assert!(txt.contain_s("nyx_test_counter"));
}

