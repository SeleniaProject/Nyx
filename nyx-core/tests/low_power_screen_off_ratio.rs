#![forbid(unsafe_code)]
// このファイルは nyx-mix 側テストの重複を避けるため最小限に縮退させたプレースホルダです。
// 仕様セクション 6 のカバレッジは nyx-mix/tests/low_power_screen_off_ratio.rs が実体テストを提供。
use nyx_core::low_power::LOW_POWER_COVER_RATIO;

/// @spec 6. Low Power Mode (Mobile) (redundant placeholder)
#[test]
fn low_power_screen_off_cover_ratio_applied() {
    // 仕様上の定数範囲のみ確認 (実挙動テストは nyx-mix 側)
    assert!(LOW_POWER_COVER_RATIO <= 0.15);
}
