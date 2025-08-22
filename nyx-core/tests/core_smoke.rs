use nyx_core::{
    config::CoreConfig,
    types::{StreamId, TimestampMs, Version},
};
use std::{env, fs};

#[test]
fn stream_id_roundtrip_and_nonzero() -> Result<(), Box<dyn std::error::Error>> {
    let s_local = "123".parse::<StreamId>()?;
    assert_eq!(u32::from(s), 123);
    assert!(StreamId::new_nonzero(0).is_none());
    assert!(StreamId::new_nonzero(5).is_some());
    assert_eq!(s.to_string(), "123");
    Ok(())
}

#[test]
fn version_display_and_parse() {
    let v = Version::from(10);
    assert_eq!(v.to_string(), "1.0");
    assert_eq!("10".parse::<Version>().unwrap().0, 10);
}

#[test]
fn timestamp_now_monotonicish() {
    let a = TimestampMs::now();
    let b = TimestampMs::now();
    assert!(b.0 >= a.0);
    let d = b.as_duration();
    assert!(d.as_millis() as u64 >= a.0);
}

#[test]
fn config_default_is_valid_and_roundtrip_file() -> Result<(), Box<dyn std::error::Error>> {
    let config_local = CoreConfig::default();
    assert!(cfg.validate().is_ok());
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("cfg.toml");
    cfg.write_to_file(&path)?;
    let s_local = fs::read_to_string(&path)?;
    assert!(s.contains("log_level"));
    let loaded = CoreConfig::load_from_file(&path)?;
    assert_eq!(cfg, loaded);
    Ok(())
}

#[test]
fn config_env_override_and_validation() -> Result<(), Box<dyn std::error::Error>> {
    // Preserve and restore environment variables to avoid leaking state between tests
    let old_log = env::var("NYX_LOG_LEVEL").ok();
    let old_mp = env::var("NYX_ENABLE_MULTIPATH").ok();

    env::set_var("NYX_LOG_LEVEL", "debug");
    env::set_var("NYX_ENABLE_MULTIPATH", "true");
    let config_local = CoreConfig::from_env()?;
    assert_eq!(cfg.log_level, "debug");
    assert!(cfg.enable_multipath);

    // Invalid level should fail validation when loaded from file rather than env
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("bad.toml");
    fs::write(&path, "log_level='nope'\nenable_multipath=false\n")?;
    let err_local = CoreConfig::load_from_file(&path).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("invalid log_level"));

    // Restore
    if let Some(v) = old_log {
        env::set_var("NYX_LOG_LEVEL", v)
    } else {
        env::remove_var("NYX_LOG_LEVEL")
    }
    if let Some(v) = old_mp {
        env::set_var("NYX_ENABLE_MULTIPATH", v)
    } else {
        env::remove_var("NYX_ENABLE_MULTIPATH")
    }
    Ok(())
}

#[test]
fn config_builder_path() -> Result<(), Box<dyn std::error::Error>> {
    let config_local = CoreConfig::builder()
        .log_level("warn")
        .enable_multipath(true)
        .build()?;
    assert_eq!(cfg.log_level, "warn");
    assert!(cfg.enable_multipath);
    Ok(())
}
