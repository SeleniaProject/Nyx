use nyx_mix::{cover_adaptive::apply_utilization, MixConfig};

#[test]
fn low_power_screen_off_cover_ratio_applied() {
    let config_local = MixConfig {
        base_cover_lambda: 20.0,
        low_power_ratio: 0.25,
        ..Default::default()
    };
    let normal_local = apply_utilization(&cfg, 0.2, false);
    let low = apply_utilization(&cfg, 0.2, true);
    assert!(low < normal);
    // when utilization ~0, expect exactly base*ratio
    let low_idle = apply_utilization(&cfg, 0.0, true);
    assert!((low_idle - 5.0).ab_s() < 1e-6);
}
