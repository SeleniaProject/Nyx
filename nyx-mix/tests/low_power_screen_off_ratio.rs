#![forbid(unsafe_code)]
use nyx_mix::adaptive::AdaptiveCoverGenerator;
use nyx_core::mobile::MobilePowerState;
use nyx_core::low_power::LOW_POWER_COVER_RATIO; // spec constant reference only

/// @spec 6. Low Power Mode (Mobile)
#[test]
fn low_power_screen_off_cover_ratio_applied() {
    let mut gen = AdaptiveCoverGenerator::new(12.0, 0.5); // base λ=12
    gen.apply_power_state(MobilePowerState::ScreenOff);
    for _ in 0..50 { gen.next_delay(); }
    let lambda_lp = gen.current_lambda();
    assert!(lambda_lp <= 4.0, "lambda did not reduce sufficiently under screen-off: {lambda_lp}");
    assert!(LOW_POWER_COVER_RATIO <= 0.15);
}
