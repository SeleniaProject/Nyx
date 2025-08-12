NyxNet 未実装/プレースホルダー総点検チェックリスト

注意: 本チェックリストは仕様書(`spec/`配下)およびコード全体を横断的に走査し、未実装・プレースホルダー・スタブ・一時無効化・将来計画を網羅的に列挙しています。各項目は原則「実装」または「撤去(不要コード)」が必要です。

グローバル(仕様・設計・README 整合)
- [x] 仕様「Windows: プロセス分離(計画)」の実装整合を取る(`spec/Nyx_Protocol_v1.0_Spec_EN.md` / `nyx-core/src/windows.rs` / `nyx-daemon/src/main.rs`)
 - [x] 仕様「Linux: seccomp-bpf」「OpenBSD: pledge/unveil」の実装整合を取る(`spec/Nyx_Protocol_v1.0_Spec_EN.md`)
- [ ] 設計「Mobile(iOS/Android): Power management, background operation(計画)」の実装方針確定(`spec/Nyx_Design_Document_EN.md` / `spec/Nyx_Design_Document.md`)
 - [ ] 設計「WebAssembly: Research」→ 実装/非対応方針を明確化(`spec/Nyx_Design_Document_EN.md`)
- [x] README の「Cover Traffic Generation(planned)」の実装/テスト反映(`README.md`)
- [ ] README の「FEC: Reed-Solomon/RaptorQ(planned)」実装状況と API 整合(`README.md`, `nyx-fec/`)
 - [x] README の「FEC: Reed-Solomon/RaptorQ(planned)」実装状況と API 整合(`README.md`, `nyx-fec/`)
- [ ] README の「0-RTT Handshake(designing)」設計→実装に昇格(`README.md`, `nyx-crypto/`, `nyx-stream/`)
- [x] README の「Sandboxing(planned)」実装反映（seccomp/pledge/unveil）(`nyx-core/src/sandbox.rs`, `README.md`)
- [ ] README の「Formal Verification(in development)」のコード/CI 整合
- [ ] README の「Cloud Integration(計画)」テンプレート群の実装整備(`charts/`, `Dockerfile` 等)

nyx-core
- [x] Zero-copy: コピー計測の実装(`nyx-core/src/zero_copy.rs`: copy_overhead_ns // TODO)
 - [x] Low power: カバートラフィック送出の実処理化(placeholder を除去)(`nyx-core/src/low_power.rs`)
- [x] Zero-copy テレメトリ統合テストのプレースホルダー解消(`nyx-core/tests/zero_copy_tests.rs`)
- [ ] Zero-copy integration: mock 依存部を本実装へ置換(crypto/FEC/UDP モックの除去)(`nyx-core/src/zero_copy/integration.rs`, `manager.rs`, `telemetry.rs`)

nyx-mix
 - [x] cMix: VDF 実装(例: Wesolowski)と RSA accumulator 統合の TODO 解消(`nyx-mix/src/cmix.rs`)
- [ ] cover/adaptive: コメントのモック手順を本実装へ置換(`nyx-mix/src/cover_adaptive.rs`)

nyx-stream
 - [x] Plugin settings: プラグインレジストリ導入時のバージョン互換性チェック実装(`nyx-stream/src/plugin_settings.rs`)
 - [x] 一時無効化モジュールの再有効化と実装復帰(`nyx-stream/src/lib.rs`)
  - [x] `frame_handler` 再実装/再公開
  - [x] `integrated_frame_processor` 実装と公開
- [ ] レガシーテスト無効化解除(`feature = "legacy_tests_disabled"` を外せる状態へ)
  - [ ] `tests/integrated_frame_processor_tests.rs`: 未定義 `IntegratedFrameConfig`/`IntegratedFrameProcessor` を実装
  - [ ] `tests/frame_handler_tests.rs`: `Frame` 型とデシリアライズ実装を整備
  - [ ] `tests/flow_controller_tests.rs`: コンストラクタと関連 API を確定
- [ ] Plugin フレーム: 非 `plugin` ビルドのスタブを最小化し、実機能へ接続(`nyx-stream/src/plugin_frame.rs`)
- [ ] Plugin ハンドシェイク: 署名鍵/署名(現在 placeholder)をレジストリ管理+検証へ置換(`nyx-stream/src/plugin_handshake.rs`)
- [ ] Multipath: 公開 API/設定の forward-compat コメント箇所に対する具体化(`nyx-stream/src/multipath/`配下)
- [ ] FEC 非有効時の互換レイヤ(compat)を本実装へ統合(`nyx-stream/src/tx.rs` 他)

nyx-daemon
- [x] Event system: 未使用 API の整理 or 実配線(`nyx-daemon/src/event_system.rs` // TODO)
- [x] Health monitor: ディスク空き監視の実装(現在は "not implemented" を返却)(`nyx-daemon/src/health_monitor.rs`)
- [x] Metrics: placeholder 値/エンドポイント/uptime の実測・蓄積・エクスポート実装(`nyx-daemon/src/metrics.rs`)
- [x] main: イベントストアの実装(placeholder を廃止)(`nyx-daemon/src/main.rs`)
- [ ] Layer manager: 一時的劣化(degrade)や bypass の実処理化(現在多くが説明ログのみ)(`nyx-daemon/src/layer_manager.rs`)
- [ ] libp2p_network: プレースホルダー(値取得/PeerId/暗号/None返却など)の全面実装(`nyx-daemon/src/libp2p_network.rs`)
- [ ] path_builder.rs: Pure Rust DHT 連携の本実装と、`path_builder_broken.rs` の残存プレースホルダー整理
  - [x] `path_builder_broken.rs`: placeholder/temporary stubs の撤去 or 非ビルド化
  - [ ] DHT discovery fallback の実装(placeholder 削除)
  - [ ] ノードメトリクス更新(placeholder)の実装
- [ ] pure_rust_dht(_tcp): 値検索/ノード ID/問い合わせ系 placeholder の実装(`nyx-daemon/src/pure_rust_dht*.rs`)
- [ ] layer_recovery_test: 一時劣化ハンドリング/回復の実テスト整備(`nyx-daemon/src/layer_recovery_test.rs`)

nyx-transport
 - [x] STUN: バッファ長等の placeholder を正規実装へ(`nyx-transport/src/stun_server.rs`)
- [ ] QUIC 非有効時のスタブ群を実装または機能フラグ設計を見直し(`nyx-transport/src/lib.rs`)

nyx-fec
- [ ] RaptorQ: 仕様/統計/適応冗長の最終化と本番 API 固定(`nyx-fec/src/raptorq.rs`)
 - [x] RaptorQ: 仕様/統計/適応冗長の最終化と本番 API 固定(`nyx-fec/src/raptorq.rs`)
- [ ] SIMD feature 非有効時挙動の整備(`nyx-fec/src/lib.rs`)

nyx-crypto
- [ ] Hybrid KEM: BIKE は placeholder/unsupported → 実装方針(採用/非採用)を確定(`nyx-crypto/src/hybrid.rs`)
- [ ] Hybrid: X25519 共有鍵 placeholder 派生の本実装化(設計に応じて修正)(`nyx-crypto/src/hybrid.rs`)
- [ ] Noise: BIKE policy-disabled の恒久方針反映(`nyx-crypto/src/noise.rs`)

nyx-telemetry
- [x] OTLP: `force_flush()` の本実装化と exporter 統合(`nyx-telemetry/src/otlp.rs`)
- [ ] OpenTelemetry 統合: placeholder な固定 ID/簡易動作を正式フローへ(`nyx-telemetry/src/opentelemetry_integration.rs`)

nyx-control
- [ ] DHT: `#[cfg(not(feature = "dht"))]` スタブ群の本実装化 or 機能フラグ方針を明確化(`nyx-control/src/lib.rs`)
- [ ] Settings: JSONSchema ドラフト/バリデーションの最終化(`nyx-control/src/settings.rs`)

nyx-cli
 - [x] main_grpc_backup: リトライ最大回数を設定から取得(TODO 解消)(`nyx-cli/src/main_grpc_backup.rs`)
- [ ] main_grpc_backup: 受信ファイル機能・リアルタイムダッシュボードの placeholder 解消
- [x] i18n: `{ $var }` プレースホルダー置換の拡張(複合/整形対応)(`nyx-cli/src/i18n.rs`)

nyx-sdk
 - [x] daemon.rs: `public_key: "public-key-placeholder"` を実キーへ置換(`nyx-sdk/src/daemon.rs`)
- [ ] reconnect/retry: 一時失敗/再接続周りのポリシー最終化(仮実装/固定値除去)(`nyx-sdk/src/reconnect.rs`, `retry.rs`)
- [ ] error.rs: CLOSE/Status マッピングでの "Unimplemented"/501 取り扱いの仕様化(`nyx-sdk/src/error.rs`)

nyx-sdk-wasm
- [ ] WASM: Multipath/Plugin system 未実装 → 方針確定し順次実装(`nyx-sdk-wasm/src/lib.rs`)
- [ ] WASM: HPKE 等の公開API拡張(コメント上未公開)(`nyx-sdk-wasm/src/lib.rs`)

nyx-mobile-ffi
- [ ] iOS/Android 非対象プラットフォーム時のスタブを縮小し、モバイル機能の本実装連携(`nyx-mobile-ffi/src/ios.rs`, `android.rs`, `common.rs`)

テスト/ベンチ・一時無効化/モック/ダミー
- [ ] `#[cfg(feature = "legacy_tests_disabled")]` 系を段階的に撤去し、全テスト常時有効化(`nyx-stream/src/tests/*`, `nyx-conformance/tests/*`)
- [ ] `assert!(true)` 等のプレースホルダーテストを実仕様テストへ置換(`nyx-core/tests/low_power_mobile_ffi.rs`, `nyx-telemetry/tests/otlp_span.rs` など)
- [ ] 各所の mock/dummy/no-op を本実装に置換し、必要なら feature で明確に隔離(`nyx-core/zero_copy/*`, `nyx-core/benches/*`, `nyx-daemon/tests/*` ほか)

プロトコル/管理フレーム
- [ ] Plugin 必須未対応時の CLOSE フレーム処理は実装済だが、プラグイン検出/互換性/署名検証の全系統を確定(`nyx-stream/src/plugin_frame.rs`, `plugin_handshake.rs`)
- [ ] Capability 交渉テスト(UNSUPPORTED_CAP 0x07)は成立しているが、対応表・拡張ポリシーをドキュメント化(`nyx-conformance/tests/capability_negotiation_properties.rs`)

ネットワーク/DHT/Path
- [ ] DHT KVS, TTL, region/capability index の恒久化・API 固定(`nyx-daemon/src/pure_rust_dht*.rs`, `nyx-control/`)
- [ ] Path 構築: active probe/metrics/地理推定(placeholder)の本実装化(`nyx-daemon/src/path_builder.rs`, `path_builder_broken.rs`)

メトリクス/監視
- [ ] Daemon の全メトリクス(システム/ネットワーク/エラー/レイヤ/アラート)の収集・閾値・アクションを実データに接続(`nyx-daemon/src/metrics.rs`)
- [ ] Prometheus/OTLP へのエクスポート完全化(`nyx-daemon/src/metrics.rs`, `nyx-telemetry/`)

セキュリティ/サンドボックス
- [x] seccomp/pledge/unveil 実装と OS ごとの fallback 設計統合(`nyx-core/src/sandbox.rs` 他)
- [ ] 暗号鍵管理/キー配送/ローテーションの本運用仕様化(placeholder/固定鍵排除)(`nyx-crypto/`, `nyx-daemon/`)

デプロイ/運用
- [ ] Helm/K8s マニフェストの最終化と seccomp プロファイルの整備(`charts/nyx/*`)
- [ ] Dockerfile/CI の最適化と検証カバレッジの拡充

ドキュメント/報告
- [ ] `IMPLEMENTATION_REPORT.md` と実装/テストの同期(仕様との差分を常時最新化)
- [ ] `spec/spec_test_mapping.json` の "placeholder" 表記を解消し、完全な仕様→テスト対応を提示

補足(個別ソース箇所 抜粋)
- [x] `nyx-daemon/src/health_monitor.rs`: "Disk space check not implemented" を実処理に置換
- [ ] `nyx-daemon/src/proto.rs`: `NyxControlService { /* placeholder */ }` の機能実装
- [ ] `nyx-daemon/src/main.rs`: イベントストア/フィルタ(placeholder)の実装
 - [x] `nyx-daemon/src/libp2p_network.rs`: 値検索/署名/暗号/メッセージ処理(placeholder)の実装
- [ ] `nyx-daemon/src/pure_rust_dht_tcp.rs`: 値検索 `None // Placeholder` の実装、Ping の node_id 生成の実装
 - [x] `nyx-daemon/src/path_builder_broken.rs`: 互換スタブ/メンテ placeholder の撤去 or 代替
 - [x] `nyx-transport/src/stun_server.rs`: placeholder 長の正規算出
- [x] `nyx-core/src/zero_copy.rs`: `copy_overhead_ns: 0 // TODO` の実計測
- [ ] `nyx-stream/src/plugin_handshake.rs`: 埋め込み鍵/署名(placeholder)→レジストリ検証
- [x] `nyx-telemetry/src/otlp.rs`: `force_flush()` の実処理
 - [x] `nyx-sdk/src/daemon.rs`: `public_key-placeholder` の解消
- [ ] `nyx-cli/src/main_grpc_backup.rs`: リトライ回数設定/TUI ダッシュボード/ファイル受信の実装

