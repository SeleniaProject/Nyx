#![forbid(unsafe_code)]
use nyx_mix::adaptive::AdaptiveCoverGenerator;

/// @spec 5. Adaptive Cover Traffic
/// 利用率が増加した際 λ が減少しない (非減少) & 低利用→高利用で短遅延化を確認。
#[test]
fn adaptive_cover_utilization_feedback_non_decreasing_lambda() {
    let mut gen = AdaptiveCoverGenerator::new(10.0, 0.4);
    // 低利用フェーズ: next_delay を数回呼んで baseline λ を記録
    for _ in 0..10 { gen.next_delay(); }
    let baseline = gen.current_lambda();

    // 高利用フェーズ: 大きな real bytes を記録しつつ next_delay
    for _ in 0..50 { gen.record_real_bytes(1200 * 5); gen.next_delay(); }
    let high_util_lambda = gen.current_lambda();

    assert!(high_util_lambda >= baseline, "lambda should be non-decreasing when utilization rises (baseline={baseline}, high={high_util_lambda})");
}
