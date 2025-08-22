#[test]
fn telemetry_init_smoke() -> Result<(), Box<dyn std::error::Error>> {
    nyx_telemetry::init(&nyx_telemetry::Config::default())?;
    Ok(())
}

#[test]
fn record_and_dump_counter() -> Result<(), Box<dyn std::error::Error>> {
    nyx_telemetry::init(&nyx_telemetry::Config::default())?;
    nyx_telemetry::record_counter("nyx_test_counter", 2);
    let txt = nyx_telemetry::dump_prometheus();
    assert!(txt.contains("nyx_test_counter"));
    Ok(())
}
