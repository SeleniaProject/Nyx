NyxNet 未実装/プレースホルダー総点検チェックリスト

注意: 本チェックリストは仕様書(`spec/`配下)およびコード全体を横断的に走査し、未実装・プレースホルダー・スタブ・一時無効化・将来計画を網羅的に列挙しています。各項目は原則「実装」または「撤去(不要コード)」が必要です。

nyx-sdk-wasm
- [x] WASM: Multipath/Plugin system 未実装 → API 設計（control/query）を確定し順次実装(`nyx-sdk-wasm/src/lib.rs`)
- [x] WASM: HPKE 等の公開API拡張（wasm-safe RNG/KEMの安定化に合わせ公開）(`nyx-sdk-wasm/src/lib.rs`)
 - [x] WASM: Multipath API 拡張（固定重み/再計算/履歴取得）と Plugin Registry 拡張（一括インポート/必須切替/IDエクスポート）
 - [x] WASM: ヘッドレス向け wasm テスト追加（Multipath 選択履歴/プラグイン設定ラウンドトリップ）(`nyx-sdk-wasm/tests/wasm_smoke.rs`)

nyx-mobile-ffi
- [x] iOS/Android 非対象プラットフォーム時のスタブを縮小し、モバイル機能の本実装連携(`nyx-mobile-ffi/src/ios.rs`, `android.rs`, `common.rs`)
- [x] Android/iOS ブリッジからテレメトリ連携（開始/停止/識別ラベル注入・低電力/ネットワーク種別反映）（`nyx-mobile-ffi/src/lib.rs`, feature `telemetry`）
  - [x] Java/Obj-C からのイベント駆動フックでラベル注入強化（セッションID/端末モデル/OSバージョン）: C API `nyx_mobile_set_telemetry_label` を追加
  - [ ] Daemon サイド Prometheus への反映整合性検証（モバイル由来メトリクスの名称/単位/ラベル）

テスト/ベンチ・一時無効化/モック/ダミー
- [x] `#[cfg(feature = "legacy_tests_disabled")]` 系を段階的に撤去し、全テスト常時有効化(`nyx-stream/src/tests/*`, `nyx-conformance/tests/*`)（`nyx-conformance` は解除済み）
- [x] `assert!(true)` 等のプレースホルダーテストを実仕様テストへ置換(`nyx-core/tests/low_power_mobile_ffi.rs`, `nyx-telemetry/tests/otlp_span.rs`, `nyx-stream/tests/*`, `nyx-sdk/tests/*`)
- [x] WASM クライアント ↔ Daemon プラグイン・ハンドシェイク E2E スモーク（成功/0x07 失敗）
- [ ] 各所の mock/dummy/no-op を本実装に置換し、必要なら feature で明確に隔離(`nyx-core/zero_copy/*`, `nyx-core/benches/*`, `nyx-daemon/tests/*` ほか)

プロトコル/管理フレーム
- [x] Plugin 必須未対応時の CLOSE フレーム処理は実装済だが、プラグイン検出/互換性/署名検証の全系統を確定(`nyx-stream/src/plugin_frame.rs`, `plugin_handshake.rs`)
- [x] Capability 交渉テスト(UNSUPPORTED_CAP 0x07)は成立しているが、対応表・拡張ポリシーをドキュメント化(`spec/Capability_Negotiation_Policy.md`, `spec/Capability_Negotiation_Policy_EN.md`)

メトリクス/監視
- [ ] Daemon の全メトリクス(システム/ネットワーク/エラー/レイヤ/アラート)の収集・閾値・アクションを実データに接続(`nyx-daemon/src/metrics.rs`)
- [ ] Prometheus/OTLP へのエクスポート完全化（環境変数でのOTLP起動配線は導入済）(`nyx-daemon/src/metrics.rs`, `nyx-telemetry/`)
 - [x] Zero-Copy 集約メトリクスを `metrics` 経由で Prometheus `/metrics` に周期エクスポート（`nyx-daemon/src/zero_copy_bridge.rs`, `nyx-daemon/src/main.rs`）
 - [x] `/metrics` エンドポイント統合テスト（Zero-Copy 指標露出の検証）を追加（`nyx-daemon/tests/prometheus_metrics.rs`）
 - [x] OTLP エクスポータのスモーク（初期化→ダミースパン→安全終了・外部コレクタ不要）（`nyx-daemon/tests/otlp_export_smoke.rs`、`nyx-daemon` フィーチャ `experimental-metrics, otlp_exporter`）
 - [x] Alerts の HTTP 露出（`/api/v1/alerts/stats`, `/api/v1/alerts/analysis`）と Prometheus 連携（`nyx_alerts_active`, `nyx_alerts_resolved`, `nyx_alerts_suppressed` ほか）(`nyx-daemon/src/main.rs`, `nyx-daemon/src/prometheus_exporter.rs`)
 - [x] Mix の Adaptive Cover Telemetry を `metrics` と `nyx-telemetry` 双方に出力（`nyx-mix/src/adaptive.rs`）
 - [x] CLI: `nyx-cli` に Alerts サブコマンド追加（`alerts stats`/`alerts analysis`）し、JSON/表形式で出力（`nyx-cli/src/main.rs`）
 - [x] Mix の Adaptive Cover Telemetry を `metrics` と `nyx-telemetry` 双方に出力（`nyx-mix/src/adaptive.rs`）
 - [x] CLI: `nyx-cli` に Alerts サブコマンド追加（`alerts stats`/`alerts analysis`）し、JSON/表形式で出力（`nyx-cli/src/main.rs`）

セキュリティ/サンドボックス
- [ ] 暗号鍵管理/キー配送/ローテーションの本運用仕様化(placeholder/固定鍵排除)(`nyx-crypto/`, `nyx-daemon/`)

デプロイ/運用
- [ ] Helm/K8s マニフェストの最終化と seccomp プロファイルの整備(`charts/nyx/*`)
- [ ] Dockerfile/CI の最適化と検証カバレッジの拡充

ドキュメント/報告
- [ ] `IMPLEMENTATION_REPORT.md` と実装/テストの同期(仕様との差分を常時最新化)

補足(個別ソース箇所 抜粋)
  - [ ] `nyx-daemon/src/main.rs`: イベントストア/フィルタ(placeholder)の実装
  - [ ] `nyx-daemon/src/libp2p_network.rs`: 値検索/署名/暗号/メッセージ処理の残存 placeholder を実装
  - [ ] `nyx-cli/src/main_grpc_backup.rs`: リトライ回数設定/TUI ダッシュボード/ファイル受信の実装

