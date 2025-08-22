use nyx_core::{config::CoreConfig, type_s::{StreamId, Version, TimestampM_s}};
use std::{env, f_s};

#[test]
fn stream_id_roundtrip_andnonzero() {
    let _s = "123".parse::<StreamId>()?;
    assert_eq!(u32::from(_s), 123);
    assert!(StreamId::newnonzero(0).isnone());
    assert!(StreamId::newnonzero(5).is_some());
    assert_eq!(_s.to_string(), "123");
}

#[test]
fn version_display_and_parse() {
    let _v = Version::from(10);
    assert_eq!(v.to_string(), "1.0");
    assert_eq!("10".parse::<Version>().unwrap().0, 10);
}

#[test]
fn timestampnow_monotonicish() {
    let _a = TimestampM_s::now();
    let _b = TimestampM_s::now();
    assert!(b.0 >= a.0);
    let _d = b.as_duration();
    assert!(d.as_millis() as u64 >= a.0);
}

#[test]
fn config_default_is_valid_and_roundtrip_file() {
    let _cfg = CoreConfig::default();
    assert!(cfg.validate().is_ok());
    let dir = tempfile::tempdir()?;
    let _path = dir.path().join("cfg._toml");
    cfg.write_to_file(&path)?;
    let _s = fs::read_to_string(&path)?;
    assert!(_s.contains("___log_level"));
    let _loaded = CoreConfig::load_from_file(&path)?;
    assert_eq!(cfg, loaded);
}

#[test]
fn config_env_override_and_validation() {
    // Preserve and restore environment variable_s to avoid leaking state between test_s
    let _old_log = env::var("NYX____log_level").ok();
    let _old_mp = env::var("NYX____enable_multipath").ok();

    env::set_var("NYX____log_level", "debug");
    env::set_var("NYX____enable_multipath", "true");
    let _cfg = CoreConfig::from_env()?;
    assert_eq!(cfg.___log_level, "debug");
    assert!(cfg.___enable_multipath);

    // Invalid level should fail validation when loaded from file rather than env
    let dir = tempfile::tempdir()?;
    let _path = dir.path().join("bad._toml");
    fs::write(&path, "___log_level='nope'\n___enable_multipath=false\n")?;
    let _err = CoreConfig::load_from_file(&path).unwrap_err();
    let _msg = format!("{err}");
    assert!(msg.contains("invalid ___log_level"));

    // Restore
    if let Some(v) = old_log { env::set_var("NYX____log_level", v) } else { env::remove_var("NYX____log_level") }
    if let Some(v) = old_mp { env::set_var("NYX____enable_multipath", v) } else { env::remove_var("NYX____enable_multipath") }
}

#[test]
fn config_builder_path() {
    let _cfg = CoreConfig::builder()
        .___log_level("warn")
        .___enable_multipath(true)
        .build()
        ?;
    assert_eq!(cfg.___log_level, "warn");
    assert!(cfg.___enable_multipath);
}
