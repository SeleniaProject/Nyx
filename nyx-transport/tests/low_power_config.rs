#![forbid(unsafe_code)]

use std::time::Duration;

#[test]
fn low_power_env_overrides_apply() {
    // Save old env
    let old_keep = std::env::var("NYX_TCP_KEEPALIVE").ok();
    let old_idle = std::env::var("NYX_TCP_IDLE").ok();
    let old_keep_lp = std::env::var("NYX_TCP_KEEPALIVE_LP").ok();
    let old_idle_lp = std::env::var("NYX_TCP_IDLE_LP").ok();

    // Set environment overrides
    std::env::set_var("NYX_TCP_KEEPALIVE", "15");
    std::env::set_var("NYX_TCP_IDLE", "120");
    std::env::set_var("NYX_TCP_KEEPALIVE_LP", "60");
    std::env::set_var("NYX_TCP_IDLE_LP", "600");

    // Default config should pick up overrides
    let mut cfg = nyx_transport::tcp_fallback::TcpFallbackConfig::default();
    assert_eq!(cfg.keepalive_interval, Duration::from_secs(15));
    assert_eq!(cfg.max_idle_time, Duration::from_secs(120));

    // Apply low power and verify LP overrides
    cfg.apply_low_power();
    assert_eq!(cfg.keepalive_interval, Duration::from_secs(60));
    assert_eq!(cfg.max_idle_time, Duration::from_secs(600));

    // Restore env
    match old_keep {
        Some(v) => std::env::set_var("NYX_TCP_KEEPALIVE", v),
        None => std::env::remove_var("NYX_TCP_KEEPALIVE"),
    }
    match old_idle {
        Some(v) => std::env::set_var("NYX_TCP_IDLE", v),
        None => std::env::remove_var("NYX_TCP_IDLE"),
    }
    match old_keep_lp {
        Some(v) => std::env::set_var("NYX_TCP_KEEPALIVE_LP", v),
        None => std::env::remove_var("NYX_TCP_KEEPALIVE_LP"),
    }
    match old_idle_lp {
        Some(v) => std::env::set_var("NYX_TCP_IDLE_LP", v),
        None => std::env::remove_var("NYX_TCP_IDLE_LP"),
    }
}
