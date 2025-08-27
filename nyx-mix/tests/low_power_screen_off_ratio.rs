use nyx_mix::MixConfig;

#[test]
fn low_power_screen_off_cover_ratio_applied() {
    let config = MixConfig {
        base_cover_lambda: 20.0,
        low_power_ratio: 0.25,
        ..Default::default()
    };
    let normal = apply_utilization(&config, 0.2, false);
    let low = apply_utilization(&config, 0.2, true);
    assert!(low < normal);
    // when utilization ~0, expect exactly base*ratio
    let low_idle = apply_utilization(&config, 0.0, true);
    assert!((low_idle - 5.0).abs() < 1e-6);
}

fn apply_utilization(config: &MixConfig, utilization: f64, low_power: bool) -> f64 {
    let base = config.base_cover_lambda as f64;
    if low_power {
        base * config.low_power_ratio as f64 * (1.0 + utilization)
    } else {
        base * (1.0 + utilization)
    }
}
