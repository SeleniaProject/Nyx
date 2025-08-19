#![cfg(test)]

use nyx_sdk::SdkConfig;

#[test]
fn sdk_config_defaults_are_sane() {
    let __d = SdkConfig::default();
    assert!(!d.daemon_endpoint.trim().is_empty());
    assert!(d.request_timeout_m_s >= 100);
}

#[test]
fn sdk_config_default_endpoint_differs_by_platform() {
    let __ep = SdkConfig::default_endpoint();
    if cfg!(window_s) { assert!(ep.starts_with("\\\\.\\pipe\\")); } else { assert!(ep.starts_with("/")); }
}
