# 未実装 / プレースホルダー / 要対応項目 集約リスト

本ファイルは仕様書 (Nyx Protocol v1.0 Spec / Design Document) とコードベース突き合わせ、およびソース内 TODO / placeholder コメント走査結果を統合した技術的バックログ。優先度や依存関係は未付与。継続的に更新すること。

## 収集方法概要
1. `TODO`, `FIXME`, `placeholder` キーワードおよび `unimplemented` を全 Rust ソースから grep。
2. 仕様 v1.0 で要求されているがコード上で未確証/未完備と思われる領域を差分分析。
3. 一部ファイル内容を確認し具体的状態を要約。

---
## A. 暗号 / ハンドシェイク関連 (nyx-crypto)
- [x] HybridNoiseHandshake 基本フロー実装: 3 メッセージ (Initiator First / Responder Second / Initiator Third) で X25519 + Kyber 部分鍵結合 (`combine_keys` / 方向性 KDF) テスト (`test_simple_hybrid_flow`) 通過。
	- 静的鍵: X25519 `StaticSecret` 導入済 (classic feature 有効時)。
	- Kyber: 現行は KEM 部分 + ハイブリッド KDF (blake3 chaining_key + HKDF) 方向鍵 send/recv を導出。
- [x] HPKE 再鍵テスト: `hpke_rekey_roundtrip` で連続 encapsulation の鍵独立性検証。
- [x] PCR 自動リキー: `pcr.rs` ポリシーベース (時間/パケット) 実装。
- [x] Noise ハイブリッド placeholder 除去: Kyber CT + X25519 eph レイアウト / transcript hash 初期処理。
- [x] Hybrid transcript 部分強化: chaining_key (blake3) + 方向性 send/recv key 導出 (暫定 Noise-like)。
- [x] ネガティブ / ロバストテスト: 長さ不足 / 再送 (replay) / Kyber CT 改竄 (成功か失敗か双方受容し divergence 検知) / Forward secrecy (ephemeral wipe) を追加。
- [x] PQ-only ビルド互換性: `HybridFailed` 常時定義 / `derive_session_key` を classic feature にゲート (panic stub→将来整理)。
- [x] PQ-Only セマンティクス強化: panic stub 排除しビルド時コンパイルエラー化 (non-classic で `derive_session_key` 未定義) 済。
- [x] Hybrid transcript / hash 完全化: 第二/第三メッセージ暗号化 + length prefix, Kyber+DH 組合せ後 mix_key で symmetric 導出。
- [x] BIKE 方針確定: PqAlgorithm::Bike 選択時は即エラー (policy-disabled) とし実装保留を明示。
- [x] Kyber CT 改竄テストの統計的安定化: 複数 (最大16) 改竄試行で divergence/エラー少なくとも1件確認するテスト追加。
- [x] Forward secrecy 拡張試験: 12 回独立ハンドシェイクで send/recv 鍵の重複無し (方向整合性含む) を検証。

## B. ストリーム / プラグイン / マルチパス (nyx-stream)
- [x] Plugin セキュリティ検証: `validate_plugin_security` に Ed25519 署名 / バージョン範囲 / capability チェック実装 & テスト (`test_security_validation` 拡張)。
- [x] Plugin 初期化: 初期化抽象 `PluginInitializer` / `InProcessPluginInitializer` 導入、失敗シミュレーション & 成功/失敗テスト (`test_plugin_initialization_failure_maps_to_incompatible_requirements`).
- [x] Capability Negotiation 異常系: 未サポート必須プラグインで失敗 (`test_unsupported_required_plugin_handshake_failure`) + CLOSE 0x07 相当差集合テスト (`test_close_on_unsupported_required_plugin`).
- [x] Multipath: 高度スケジューラ (RTT/帯域/損失) 動的重み付け + グローバル/パス別 ReorderingBuffer 統合実装 & テスト。
- [x] Reordering Buffer: per-path & グローバル再順序復元ロジック + 統合テスト (`test_global_reorder_across_paths`).
 - [x] HPKE Rekey: 基本ポリシー (時間/パケット) / outbound フレーム生成 / inbound 解析・鍵適用 / 失敗 & 解析異常テスト / クールダウン(min_cooldown) / async flush API / 制御(CRYPTO)チャネル自動送出補助 (TxQueue::send_all_rekey_frames_via) / cooldown 抑止カウンタ / key lifetime histogram / 旧キー grace 終了イベント通知 (grace_notifier) 実装完了。残: FeatureMatrix 組合せ検証 (@spec テスト追加で完了予定)。
 - [x] WRR: weight_ratio_deviation Gauge 追加 (期待比 vs 実送信重み乖離)。
 - [x] Multipath: per-path RTT jitter Histogram 追加 & 再順序バッファ適応サイズ計算フィード実装 (adaptive_min / adaptive_max / 利用率 & 遅延メトリクス追加)。

## C. Mix / cMix / カバートラフィック (nyx-mix)
- [x] `cmix.rs` accumulator integration: `BatchProof` 構造体 / `verify_batch_detailed` 追加し RSA アキュムレータ + VDF 検証を構造化エラーで返却。
- [x] cMix VDF 処理: Wesolowski 実装 + `prove_mont` 利用し iteration count による遅延。 校正ヘルパ `calibrate_t` + タイミング検証テスト (許容範囲内) 追加。
- [x] Adaptive Cover Traffic: 利用率バンド U∈[0.2,0.6] の EMA ベース λ 調整 (band controller) 実装 / テストで低利用→高利用で λ 非減少を検証。

## D. トランスポート (nyx-transport)
- [x] QUIC DATAGRAM: Quinn ラッパ `quic.rs` + datagram roundtrip テスト追加 (feature `quic` 下)。
- [x] TCP フォールバック: `tcp_fallback.rs` 既存実装 + 統計/再試行/フレーム境界テスト網羅済を確認。
- [x] NAT Traversal: ICE-lite ヘルパ (`ice.rs`) + Teredo クライアント (`teredo.rs`) 最小実装 / STUN 応答デコードテスト追加。
- [x] STUN サーバ: レスポンスビルダー関数化・length/type placeholder 解消 / ラウンドトリップテスト追加。

## E. FEC (nyx-fec)
- [x] RaptorQ: `raptorq.rs` 完全実装 (sentinel 先頭配置 / 自適応 `AdaptiveRaptorQ` / 再計算ロジック) + エンコード/デコード/適応/統計/セッション清掃テスト整備。
- [x] タイミング難読化: `timing.rs` の `TimingObfuscator` 動作テスト (遅延分布上限) 追加。
- [x] 固定パケット長: `padding.rs` 丸め込み + 復元 roundtrip テスト追加 (1280B 境界整合性確認)。

## F. コントロールプレーン / DHT / ルーティング (nyx-control, nyx-daemon)
- [x] DHT: `pure_rust_dht.rs` InMemoryDht (TTL / region / capability index) + 基本テスト。
- [ ] Path Builder 拡張: 追加高度機能の一部残 (帯域アクティブ測定, さらなる多様性最適化調整)。
	- [x] InMemoryDht 連携 (Region インデックス取得)。
	- [x] Discovery Criteria: Region / Capability / Random / All / Latency + 内部キャッシュ。
	- [x] テスト: `capability_and_random_all_discovery` / region_discovery / topology 更新 / probing メトリクス。
	- [x] DHT 結果 → グラフノード / 自動エッジ生成 (latency+距離閾値) 反映。
	- [x] 背景タスク: discovery / probing / cache maintenance 追加 (spawn)。
	- [x] プロービング: 疑似値 + オプション実TCPコネクト RTT 測定 (enable_real_probing フラグ) 実装。
	- [x] キャッシュ aging + usage_count / last_access + 周波数 decay (lfu_decay) 付き簡易 LFU 実装。
	- [x] Reputation 永続化 & スコア重み付け統合 (JSON 保存 / ReputationStore)。
	- [x] PathQuality 再計算 (update_node_metrics -> recompute_impacted_paths) + Push 通知連携。
	- [x] 地理多様性スコア: 平均距離 + 距離分散/クラスタペナルティ + 地域エントロピー複合指標へ改良。
	- [x] Capability Catalog: mandatory/optional 管理 & mandatory 強制フィルタリング適用。
	- [x] 帯域リアル計測 (TCP転送/サンプリング) 拡張 (接続 RTT + サンプル帯域/中央値算出フレーム; 実バースト測定フック済)。
	- [x] 経路多様性さらなる最適化 (動的重み調整: EMAヒューリスティック学習 + 多要素統計適用)。
- [x] Push Notifications: 内部 PushManager による path_quality チャンネル発行 & テスト。
- [x] Capability Management / Feature Flags: 集中カタログ + 交渉/拒否 (mandatory 欠如ノード除外) 実装。

## G. デーモン (nyx-daemon)
- [x] Event System: 未使用API の可視性縮小（get_queue_size / get_subscriber_info を pub→pub(crate)）＆ session lifecycle イベント統合。今後: 追加統合/更なる削減余地。
- [x] libp2p_network:
	- [x] 認証リクエスト送信: 純TCP + length prefix (bincode) 実装。
	- [x] 暗号化メッセージ送信: nonce(12B)+ciphertext length prefix 送出実装。
- [x] metrics.rs: ディスク使用量 / FD / thread count クロスプラットフォーム(Unix/Windows) ヒューリスティック実装。精度向上(Windows, 追加統計) は後続タスク化。
- [x] health_monitor.rs: uptime_seconds / active_connections 取得ロジックと accessor 注入実装。
- [x] alert_system.rs: Email (簡易SMTP: HELO/Mail/RCPT/DATA) / Webhook (HTTP POST) 実装。TLS/認証は保留。
- [x] stream_manager.rs: target_address, packets_sent/received, path_stats 埋め込み & PathStat 生成。
- [x] power / low power mode: モバイルFFI (battery/screen/app_state/OS低電力) 連動 + ライフサイクル別動的評価 (inactivity閾値縮小) + manual override + power.state.changed イベント発火 + 推奨 cover 比率(低:0.1/通常:1.0) + adaptive interval (Active30s/Background15s/Inactive10s)。後続: 端末固有最適化/cover直接制御/設定永続化。

## H. CLI (nyx-cli)
- [x] NodeInfo JSON / YAML 出力実装 (`cmd_status`) 完了 (pure + legacy gRPC)。
- [x] 再接続試行最大回数: `nyx.toml` `[cli].max_reconnect_attempts` から読込実装。pure CLI で適用 / legacy 版は feature `grpc-backup` 時も config 読込対応。
- [x] `main_grpc_backup.rs` を legacy 化 (feature `grpc-backup` + 冒頭コメントで保守方針明示 / アーカイブ位置づけ)。

## I. SDK / API / WASM
- [x] WASM crate (`nyx-sdk-wasm`) 機能差異ドキュメント (lib.rs ヘッダ + `docs/SDK_WASM_FEATURE_MATRIX.md`) 追加。
- [x] SDK エラー型: Close コード (UNSUPPORTED_CAP 等) へのマッピング `close_code()` 実装 + テスト追加。

## J. Telemetry / 観測性 (nyx-telemetry)
- [x] OpenTelemetry: スパン名 / 属性実装・検証済。
- [x] System metrics 収集 (CPU/Mem/Disk%/Network)。
- [x] Tracing ↔ Metrics 相互紐付けテスト。
	- [x] インメモリ span キャプチャ + 決定的サンプリング + 属性フィルタ (hot reload JSON / multi-chain)。
	- [x] 手動 OTLP exporter (tonic) + recovery(backoff + circuit breaker) + 健全性メトリクス登録。
	- [x] OTLP E2E コレクタ統合テスト (manual exporter パス)。
	- [x] サンプリング統計カウンタ (kept/dropped) 公開。
	- [x] 属性フィルタ拡張 / ランタイム差し替え。
	- [x] Exporter エラー回復メトリクス (success/failure/circuit)。
	- [x] Span→Metric 統合 E2E (path_id span 出現で counter 増加)。
 - [x] Plugin 初期化メトリクス: 成功/失敗/セキュリティ結果カウンタ & per-plugin 初期化時間 histogram 観測統合済。
 - [x] HPKE 再鍵メトリクス拡張: key lifetime histogram。
 - [x] HPKE 再鍵メトリクス拡張: cooldown 抑止回数 (suppressed) カウンタ。
 - [x] HPKE 再鍵メトリクス拡張: failure 内訳(reason label) カウンタ。
 - [x] Multipath Telemetry: weight_ratio_deviation Gauge 登録 (B との重複トラッキング)。
 - [x] Multipath Telemetry: per-path RTT jitter Histogram 登録 (B との重複トラッキング)。
 - [x] Multipath Telemetry: reorder_delay Histogram 登録。
 - [x] Multipath Telemetry: reorder_buffer_utilization Gauge 登録。
 - [x] Push Gateway Telemetry: wake / debounced_wake / reconnect_{success,fail} カウンタ追加。
 - [x] Push Gateway Latency 分位点: p50 / p95 簡易リングバッファ実装 (histogram 本実装は後続)。
 - [x] Low Power: suppressed_cover_packets 推定カウンタ + telemetry 追加。
 - [ ] Push Gateway: ジッタ付き backoff / 遅延 full histogram (bucket化) 導入。

## K. セキュリティ / サンドボックス
- [x] Windows sandboxing: JobObject 拡張 (UI 制限, 失敗時 telemetry warn ログ) 実装。残: token 権限縮小 / ACL ルート制限 (今後強化候補)。
- [x] Plugin security validation: `plugin_handshake.rs` に Ed25519 署名検証 + バージョン範囲 + capability チェック追加（簡易レジストリ/埋込キー）。
- [x] Zeroization Phase 1: 主要鍵型 (SessionKey / HybridSecretKey / SharedSecret / NoiseTransport / EncryptionContext / PCR rekey) へ ZeroizeOnDrop / Drop 実装 & 監査文書。
 - [x] Zeroization Phase 2: ランタイム検証 (PCR 再鍵旧キーゼロ化テスト追加) / feature hybrid 条件付テスト雛形。Miri 深度検証は将来オプション化。

## L. テスト / 検証 / フォーマル
- [x] Hybrid handshake 試験 (ignore 解除)。
- [x] Multipath end-to-end (経路同時利用/フェイルオーバ) テスト。
 - [x] HPKE 再鍵制御統合テスト (自動送出→受信→適用フロー、grace 窓内/外 decrypt パス) : TxQueue pending→send_all_rekey_frames_via + manager grace_notifier 実装 / すべて @spec (3,9) ラベル付与済。
 - [x] HPKE 再鍵異常系テスト (パース失敗 / 復号失敗)。
- [x] Plugin Handshake 異常系 (欠落必須 / 初期化失敗 / 署名検証) テスト追加。
- [x] Capability Negotiation 未サポート必須 -> 0x07 CLOSE 相当テスト追加。
- [x] Adaptive cover traffic 利用率フィードバック テスト。
- [x] Low Power Mode: 画面オフシミュレーション & cover_ratio=0.1。
- [x] FEC: RaptorQ 冗長率適応テスト。
- [x] Telemetry: スパン属性/サンプリング/統合 E2E テスト。
- [x] Disk/FD/Thread metrics クロスプラットフォーム検証。
- [x] Formal (TLA+) Multipath 拡張仕様同期精査 (初回同期メモ追加)。

## M. ドキュメント / スペック同期
- [x] v1.0 Spec Draft-Complete 反映: 差分一覧を Documentation へ統合 (CHANGELOG / IMPLEMENTATION_REPORT 更新完了)。
- [x] Plugin Frame 0x50–0x5F: 具体バイトレイアウト & CBOR スキーマ定義 + 自動生成 (`generate_plugin_schema` バイナリ) 実装。
- [x] Compliance Levels (Core / Plus / Full): 判定ロジック (`nyx_core::compliance::determine`) Daemon 統合 & CLI 表示機能。
- [x] Mobile Power モード / Push通知 統合ガイド追加 (`MOBILE_POWER_PUSH_INTEGRATION.md`) & Peer Auth ガイドリンク更新。

## N. パフォーマンス最適化
 - [x] Multipath スケジューラ: RTT / 帯域 / 損失 / ジッタ複合動的重み + 再順序バッファ適応サイズ (RTT/帯域/平均パケット長/ジッタ) 実装完了。
 - [x] Multipath 追加最適化: burst loss 検知ペナルティ / エントロピー(公平性) 指標 / 重み再計算クールダウン閾値細分化（全て実装 & テレメトリ統合）。
 - [x] Reorder Buffer 高度化: p95 遅延サンプリング + PID サイズ調整 (グローバル / パス別) & テスト追加。
 - [x] Weighted Round Robin: スロット複製方式→Smooth Weighted Round Robin へ刷新し分布安定化 (テスト安定)。
 - [x] HPKE 再鍵オーバーヘッド分析: Criterion ベンチ + 解析スクリプト (CSV 出力) 追加。
 - [ ] Zero-copy: クリティカルパス (暗号→FEC→送信) の end-to-end コピー / 再割当総数計測は未完 (AEAD 余剰アロケ / RaptorQ encode トレースのみ) → 集約カウンタ & 削減策 TODO。
 - [x] Cover traffic レート制御: 適応 Poisson λ (利用率バンド + 匿名性スコア) 実装済 / PPS & 比率偏差メトリクス追加。

## O. エラーハンドリング / API 整合性
 - [x] gRPC API: 未使用/未実装メソッド差分監査スクリプト (proto / trait / impl / 参照走査) 追加。
 - [x] Error code -> User facing メッセージ i18n: 監査スクリプト拡張 & 欠落キー (unsupported-cap / resource-exhausted / failed-precondition) 各言語追加。
 - [x] CLOSE コード (UNSUPPORTED_CAP 等) → gRPC 抽象カテゴリ変換ポリシー定義 (daemon + SDK ヘルパ)。
 - [ ] CLOSE コード変換の CLI / FFI 実利用統合 (ユーザ向けエラー出力反映) – 未着手。

## P. モバイル / 低電力
- [x] Low Power Mode: 実環境 (Android/iOS) トリガ連動 (画面オン/オフ, バッテリレベル) 実装/FFI 層公開。
	- FFI ポーリング検出器 (mobile_ffi) 統合 / screen state channel 監視。
	- Battery 取得 / PowerSave / CriticalBattery 判定 & cover traffic 動的調整。
	- Telemetry (feature `telemetry`) カウンタ: cover_packets / push_notifications。
	- ScreenOn 遷移時イベント発火 & state transition 統計収集。
	- 統合テスト: cover 生成 / CriticalBattery 遷移 / 自動再開 (push gateway 連動) 追加。
- [x] Push Notification Path: Gateway 経路確立/再接続フォールバック実装。
	- PushGatewayManager: wake デバウンス(2s) + 指数バックオフ(200ms base, 最大5試行)。
	- FFI: nyx_push_wake / nyx_resume_low_power_session 実装。
	- LowPowerManager へ attach_push_gateway() による連結 + ScreenOn で自動 resume spawn。
	- 統計 API: total_wake_events / debounced_wake_events / total_reconnect_{attempts,failures,success} / avg_reconnect_latency_ms / p50 / p95。
	- 統合テスト: ScreenOff→On で resume 起動検証。
	- Telemetry: wake / debounced / reconnect 成功・失敗カウンタ導入済。
	- 追加計画 (未実装・今後): ジッタ付 backoff 上限 / full latency histogram (分位点→bucket) / push latency outlier アラート。
	- Low Power: suppressed_cover_packets 推定カウンタ実装 (intensity 比率から算出)。

## Q. ビルド / CI
- [x] Feature Matrix テスト (代表組合せ smoke: base / hpke / hpke+telemetry / plugin / mpr_experimental / fec) 追加 (`tests/feature_matrix.rs`)。残: hybrid / pq_only (別クレート feature 分岐) 拡張と CI 行列化。
- [ ] Windows 特有 placeholder (load average, disk metrics) 実装 or 適切な conditional skip。
 - [x] Feature 警告解消: unexpected cfg (prometheus / dynamic_config / mpr_experimental / hybrid / cmix / plugin / low_power / grpc-backup) 用空 feature 宣言追加。

## R. トレーサビリティ / 自動化
- [x] spec_diff: セクションハッシュ + uncovered keyword リスト出力実装。
- [x] キーワードカバレッジ 100% (v1.0) 達成。
- [x] spec_test_map: 重複排除 / セクションカバレッジ(%) 出力ブロック自動生成。
- [ ] セクションカバレッジ 100% 達成 (現状 90%: cMix integration テスト検出漏れ解消)。
- [ ] cMix 内部テスト検出: spec_test_map の glob / 解析拡張で内部 #[cfg(test)] をマップ。
- [ ] spec_diff へ section_coverage_percent / unmapped_sections 連携統合。
- [ ] CI ゲート: keyword >=95%, section ==100% 未満で失敗するジョブ追加。
- [ ] SPEC_TEST_MAPPING.md 自動生成: unmapped セクション解消後安定化 & 手動記述部と生成部分離明確化。
- [ ] Low Power Mode 詳細テスト (画面オフ / cover_ratio=0.1) 完了後 @spec 付与しカバレッジ反映 (L セクションと連動)。
- [ ] HPKE 再鍵 E2E 制御統合テスト (@spec) 追加し metrics も観測 (L / J と連動)。
- [ ] Adaptive cover traffic 利用率フィードバック テスト @spec 付与 (L と連動)。

---
## ソース内直接 TODO / Placeholder 抜粋 (抜けがあれば随時追加)
- (整理済 / 完了のため削除対象) nyx-mix/src/cmix.rs: accumulator integration → 実装完了。
- (整理候補) nyx-daemon/src/event_system.rs: 未使用 API さらなる prune 余地。
- nyx-daemon/src/libp2p_network.rs: 実 TCP 送信部分追加検証 (仕様上は完了扱い、コード確認で残タスクあれば再分類)。
- nyx-daemon/src/metrics.rs: Windows 精度改善 / 追加統計 (既存最小実装→強化 TODO)。
- nyx-daemon/src/health_monitor.rs: 現在取得実装済 (コメント要同期: placeholder 表記削除予定)。
- nyx-daemon/src/alert_system.rs: TLS/認証 拡張保留 (基本機能完了)。
- nyx-daemon/src/stream_manager.rs: 埋め込み済 (コメント清掃)。
- nyx-crypto/src/noise.rs: HybridHandshake 追加コンストラクタ/remote key API 整理（仕様再精査）。
- nyx-stream/src/plugin_handshake.rs: 実プラグインシステム接続 (現状スタブ) – 実装計画要。
- nyx-stream/src/mpr.rs: experimental → プロダクション昇格レビュー。
- path_builder_broken.rs: 歴史的ファイル (要: 各 placeholder の現行 path_builder 反映/削除方針)。
- telemetry: logger placeholder / system metrics コメントは実装済部分と同期し dead comment 削除。
- transport/stun_server.rs: 現行実装済 (placeholder コメント削除)。
  
【新規追加】
- (整理済) nyx-stream HPKE: control channel 自動 flush 補助 API 追加済。
- (整理済) nyx-stream HPKE: cooldown 抑止カウンタ & key lifetime histogram 実装済。
- ビルド警告 cleanup: unexpected cfg(feature="telemetry") @ nyx-crypto/pcr.rs 他 unused import 削除。
- ドキュメント: HPKE_REKEYING.md に cooldown / async flush / 異常系テスト 追記。

---
## 次ステップ提案 (高インパクト順の一例)
1. HPKE 再鍵: control channel 自動送出統合 + telemetry 拡張 (cooldown_suppressed / key_lifetime) + E2E テスト。
2. Multipath: ReorderingBuffer 実装と高度スケジューラ (RTT/帯域/損失) プロトタイプ + ベンチ。
3. 観測性: デーモン Windows metrics 精度向上 / Feature Matrix CI / ビルド警告除去。
4. HybridHandshake ignore 解除 & フォーマル仕様 (TLA+) Multipath 拡張同期精査。
5. Plugin 実プラグインシステム接続 (loader / IPC 拡張) とセキュリティ registry 外部署名マニフェスト化。
6. ドキュメント & Compliance: 差分一覧 / Compliance Level 判定ロジック + CLI 出力 / HPKE ドキュメント追補。
7. パフォーマンス: Zero-copy 計測パス導入→再割当削減案適用、カバートラフィック適応検証テスト。

---
更新日: 2025-08-11 (Telemetry/Multi-path/Traceability 拡張更新)
更新再反映: 2025-08-11 (Performance & Error Handling セクション進捗反映 / Zero-copy 残タスク明確化)
更新再反映: 2025-08-11 (PushGateway telemetry / suppressed cover metric / Smooth WRR / feature 警告整理 反映)

