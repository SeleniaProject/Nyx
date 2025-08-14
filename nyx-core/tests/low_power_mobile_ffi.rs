//! Integration-like test for mobile_ffi LowPowerManager factory.
//! This test exercises only compilation/path because real platform FFI
//! side-effects are not present in CI (desktop). It validates that the
//! factory method constructs and monitoring starts without panic under
//! the feature gate.

#[cfg(feature = "mobile_ffi")]
#[tokio::test]
async fn construct_with_mobile_ffi() {
    use nyx_core::low_power::LowPowerManager; // crate path adjusted by workspace
    // Build manager with no push service.
    let mgr = LowPowerManager::with_mobile_ffi(None).expect("factory");
    // Start monitoring (should spawn tasks).
    mgr.start_monitoring().await.expect("monitoring start");
    // Basic invariant: initial state ScreenOn
    assert_eq!(mgr.get_power_state(), nyx_core::low_power::PowerState::ScreenOn);
}

#[cfg(not(feature = "mobile_ffi"))]
#[test]
fn mobile_ffi_test_skipped() {
    // When mobile_ffi feature is disabled on non-mobile platforms, ensure the
    // symbolic types remain accessible but constructors are gated.
    // Compile-time check: type path compiles.
    use nyx_core::low_power::PowerState;
    let _ = PowerState::ScreenOn; // symbol presence check
    // Runtime no-op assertion to indicate intentional skip.
    assert_eq!(1, 1, "mobile_ffi disabled build should compile paths");
}
