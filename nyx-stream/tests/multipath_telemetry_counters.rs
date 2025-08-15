#![cfg(feature = "telemetry")]
#![forbid(unsafe_code)]

use nyx_core::config::MultipathConfig;
use nyx_stream::multipath::manager::MultipathManager;
use prometheus::default_registry;
use std::time::Duration;

#[tokio::test]
async fn multipath_activation_deactivation_telemetry() {
    let reg = default_registry();
    nyx_telemetry::ensure_multipath_metrics_registered(reg);

    let config = MultipathConfig::default();
    let manager = MultipathManager::new(config);
    // Add and remove paths
    manager.add_path(10).await.unwrap();
    manager.add_path(11).await.unwrap();
    manager.add_path(12).await.unwrap();
    manager.remove_path(11, "test".into()).await.unwrap();
    // RTT updates
    manager
        .update_path_rtt(10, Duration::from_millis(10))
        .await
        .ok();
    manager
        .update_path_rtt(12, Duration::from_millis(25))
        .await
        .ok();
    // Stats snapshot updates active paths gauge
    let _ = manager.get_stats().await;

    let families = prometheus::gather();
    let mut act = 0f64;
    let mut deact = 0f64;
    let mut active_g = None;
    let mut hist = false;
    for f in families {
        match f.get_name() {
            "nyx_multipath_path_activated_total" => {
                if let Some(m) = f.get_metric().get(0) {
                    act = m.get_counter().get_value();
                }
            }
            "nyx_multipath_path_deactivated_total" => {
                if let Some(m) = f.get_metric().get(0) {
                    deact = m.get_counter().get_value();
                }
            }
            "nyx_multipath_active_paths" => {
                if let Some(m) = f.get_metric().get(0) {
                    active_g = Some(m.get_gauge().get_value());
                }
            }
            "nyx_multipath_path_rtt_seconds" => {
                hist = true;
            }
            _ => {}
        }
    }
    assert!(act >= 3.0, "expected >=3 activations got {act}");
    assert!(deact >= 1.0, "expected >=1 deactivation got {deact}");
    let g = active_g.expect("active gauge missing");
    assert_eq!(g as i64, 2, "expected 2 active paths got {g}");
    assert!(hist, "RTT histogram not observed");
}
