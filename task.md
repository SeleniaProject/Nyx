NyxNet 未実装/プレースホルダー総点検チェックリスト

注意: 本チェックリストは仕様書(`spec/`配下)およびコード全体を横断的に走査し、未実装・プレースホルダー・スタブ・一時無効化・将来計画を網羅的に列挙しています。各項目は原則「実装」または「撤去(不要コード)」が必要です。

nyx-core
- [x] Zero-copy integration: mock 依存部を本実装へ置換(crypto/FEC/UDP モックの除去)(`nyx-core/src/zero_copy/integration.rs`, `manager.rs`, `telemetry.rs`)

nyx-mix
- [x] cover/adaptive: コメントのモック手順を本実装へ置換(`nyx-mix/src/cover_adaptive.rs`)

nyx-stream
- [x] Plugin フレーム: 非 `plugin` ビルドのスタブを最小化し、実機能へ接続(`nyx-stream/src/plugin_frame.rs`)
- [x] Multipath: 公開 API/設定の forward-compat コメント箇所に対する具体化(`nyx-stream/src/multipath/`配下)
- [x] FEC 非有効時の互換レイヤ(compat)を本実装へ統合(`nyx-stream/src/tx.rs` 他)

nyx-daemon
- [x] Event system: 未使用 API の整理/実配線を完了し、残存 TODO を撤去(`nyx-daemon/src/event_system.rs`)
- [x] Layer manager: 一時的劣化(degrade)や bypass の実処理化(現在多くが説明ログのみ)(`nyx-daemon/src/layer_manager.rs`)
- [x] libp2p_network: プレースホルダー(値取得/PeerId/暗号/None返却など)の全面実装(`nyx-daemon/src/libp2p_network.rs`)
  - [x] libp2p_network: プレースホルダー(値取得/PeerId/暗号/None返却など)の全面実装と残存 placeholder の解消(`nyx-daemon/src/libp2p_network.rs`)
- [x] path_builder.rs: Pure Rust DHT 連携の本実装と、`path_builder_broken.rs` の残存プレースホルダー整理（`DummyDhtHandle` 永続化、`update_peer_info` の DHT region/cap インデックス更新、`enhanced_peer_discovery` と `update_network_topology_from_dht_peers` 強化）
  - [x] DHT discovery fallback の実装(placeholder 削除)
  - [x] ノードメトリクス更新(placeholder)の実装
- [x] pure_rust_dht(_tcp): 値検索/ノード ID/問い合わせ系 placeholder の実装(`nyx-daemon/src/pure_rust_dht*.rs`)
- [x] layer_recovery_test: 一時劣化ハンドリング/回復の実テスト整備(`nyx-daemon/src/layer_recovery_test.rs`)
 - [x] metrics: 一時的アラートシステム(temporary)を本番アラートへ置換(`nyx-daemon/src/metrics.rs`)
 - [x] push: Push 通知モジュールの mock を実実装へ置換（FCM/APNS 抽象の確立）(`nyx-daemon/src/push.rs`, `nyx-daemon/src/lib.rs`)
 - [x] low_power: デスクトップ stub を実機能へ拡張（モバイル電源状態イベント連携）(`nyx-daemon/src/low_power.rs`)

nyx-transport
- [x] QUIC 非有効時のスタブ群を実装または機能フラグ設計を見直し(`nyx-transport/src/lib.rs`)
  - [x] UDP 受信経路をデーモンへ配線（`Transport` → `DaemonPacketHandler` → `StreamManager::route_incoming`）

nyx-fec
- [x] RaptorQ: README/ドキュメント/ベンチマークの最新化（実装済 API と一致）(`nyx-fec/src/raptorq.rs`, `README.md`, `docs/`)
- [x] SIMD feature 非有効時挙動の整備(`nyx-fec/src/lib.rs`)

nyx-crypto
 - [x] Hybrid KEM: BIKE は placeholder/unsupported → 実装方針(非採用)を確定（feature指定時は明確にコンパイルエラー、APIはUnsupportedAlgorithmを返す）(`nyx-crypto/src/hybrid.rs`, `nyx-crypto/src/noise.rs`)
- [x] Hybrid: X25519 共有鍵 placeholder 派生の本実装化(設計に応じて修正)(`nyx-crypto/src/hybrid.rs`)
- [x] Noise: BIKE policy-disabled の恒久方針反映(`nyx-crypto/src/noise.rs`)
 - [x] Noise: Kyber 併用時の一時キー/リモート鍵 placeholder を実装に置換（テスト専用処理の排除）(`nyx-crypto/src/noise.rs`)

- nyx-control
- [x] DHT: `#[cfg(not(feature = "dht"))]` スタブ群の本実装化 or 機能フラグ方針を明確化(`nyx-control/src/lib.rs`)
- [x] Settings: JSONSchema ドラフト/バリデーションの最終化(`nyx-control/src/settings.rs`)

nyx-cli
- [x] main_grpc_backup: 受信ファイル機能・リアルタイムダッシュボードの placeholder 解消

nyx-sdk
- [x] error.rs: CLOSE/Status マッピングでの "Unimplemented"/501 取り扱いの仕様化(`nyx-sdk/src/error.rs`)

nyx-sdk-wasm
- [ ] WASM: Multipath/Plugin system 未実装 → API 設計（control/query）を確定し順次実装(`nyx-sdk-wasm/src/lib.rs`)
- [ ] WASM: HPKE 等の公開API拡張（wasm-safe RNG/KEMの安定化に合わせ公開）(`nyx-sdk-wasm/src/lib.rs`)

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
- [x] DHT KVS, TTL, region/capability index の恒久化・API 固定（GC/インデックス自動スクラブ追加）(`nyx-daemon/src/pure_rust_dht*.rs`, `nyx-control/`)
- [x] Path 構築: active probe/metrics/地理推定(placeholder)の本実装化（DHT GC の稼働を PathBuilder 初期化時に連動）(`nyx-daemon/src/path_builder.rs`, `path_builder_broken.rs`)
  - [x] DHT bootstrap peers / 地域インデックスの永続化と再起動復元（hot/cold セット・学習スコア・TTL/GC 連動）(`nyx-daemon/src/path_builder.rs`, `nyx-daemon/src/pure_rust_dht*.rs`)

メトリクス/監視
- [ ] Daemon の全メトリクス(システム/ネットワーク/エラー/レイヤ/アラート)の収集・閾値・アクションを実データに接続(`nyx-daemon/src/metrics.rs`)
- [ ] Prometheus/OTLP へのエクスポート完全化（環境変数でのOTLP起動配線は導入済）(`nyx-daemon/src/metrics.rs`, `nyx-telemetry/`)

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
- [ ] `nyx-stream/src/plugin_handshake.rs`: 埋め込み鍵/署名(placeholder)→レジストリ検証
- [ ] `nyx-cli/src/main_grpc_backup.rs`: リトライ回数設定/TUI ダッシュボード/ファイル受信の実装

