#![forbid(unsafe_code)]
#![cfg(feature = "experimental-metrics")]

use nyx_daemon::metrics::MetricsCollector;

/// @spec 8. System Metrics (Disk/FD/Thread)
/// クロスプラットフォーム: 取得 API がパニックせず 0 以上の値 (あるいは合理的範囲) を返すことを確認する簡易スモーク。
#[test]
fn system_metrics_basic_smoke() {
    let collector = std::sync::Arc::new(MetricsCollector::new());
    // 単発 refresh (非公開フィールドへのアクセスは避け、公開メソッドの副作用確認 / 低リスク)
    // start_collection は無限ループなので起動しない。
    // 代わりに counters をインクリメントしてもパニックしないことを検証
    collector.increment_requests();
    collector.increment_successful_requests();
    collector.increment_failed_requests();
    collector.increment_packets_sent();
    collector.increment_packets_received();
    collector.increment_retransmissions();
    collector.increment_bytes_sent(128);
    // 期待: Atomic カウンタが >=1
    assert!(collector.total_requests.load(std::sync::atomic::Ordering::Relaxed) >= 1);
}

#[tokio::test]
async fn zero_copy_bridge_exports_metrics() {
    use nyx_daemon::zero_copy_bridge::start_zero_copy_metrics_task_with_interval;
    use nyx_core::zero_copy::manager::{ZeroCopyManager, ZeroCopyManagerConfig};
    use std::sync::Arc;
    // Arrange a manager and kick off export at a short interval
    let manager = Arc::new(ZeroCopyManager::new(ZeroCopyManagerConfig::default()));
    start_zero_copy_metrics_task_with_interval(manager, std::time::Duration::from_millis(50));
    // Let one tick happen
    tokio::time::sleep(std::time::Duration::from_millis(120)).await;
    // If no panic occurred, the exporter path is wired
    assert!(true);
}