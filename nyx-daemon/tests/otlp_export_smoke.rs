#![forbid(unsafe_code)]
#![cfg(all(feature = "experimental-metrics", feature = "otlp_exporter"))]

// OTLP スモーク: エクスポータ初期化→ダミースパン→クラッシュしないことを確認
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn otlp_exporter_initializes_and_emits_without_panic() {
    use nyx_telemetry::opentelemetry_integration::{NyxTelemetry, TelemetryConfig};
    // 環境変数に依存せず直接初期化
    let cfg = TelemetryConfig {
        endpoint: "http://127.0.0.1:4317".to_string(), // Collector 不在でも接続失敗はリトライし、panic しない設計
        service_name: "nyx-daemon".to_string(),
        sampling_ratio: 1.0,
    };

    // 初期化（手動エクスポータワーカー起動）
    let _ = NyxTelemetry::init_with_exporter(cfg.clone());

    // テストスパンを1つ発行
    NyxTelemetry::test_span();

    // 短時間待機してバックグラウンドワーカー処理を進める
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 明示的シャットダウン（多重呼び出し安全）
    NyxTelemetry::shutdown();
}


