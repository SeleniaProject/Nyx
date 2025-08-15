#![forbid(unsafe_code)]

#[tokio::test]
async fn mix_defaults_follow_spec_or_env() {
    use nyx_core::config::NyxConfig;
    // Without env overrides, defaults should be Batch=100, VDF=100ms via nyx-core config
    std::env::remove_var("NYX_CMIX_BATCH");
    std::env::remove_var("NYX_CMIX_VDF_MS");
    let cfg = NyxConfig::default();
    assert_eq!(cfg.mix.batch_size, 100);
    assert_eq!(cfg.mix.vdf_delay_ms, 100);

    // With env overrides, they should apply
    std::env::set_var("NYX_CMIX_BATCH", "128");
    std::env::set_var("NYX_CMIX_VDF_MS", "150");
    let cfg2 = NyxConfig::default();
    assert_eq!(cfg2.mix.batch_size, 128);
    assert_eq!(cfg2.mix.vdf_delay_ms, 150);
}
