### NyxNet v1.0 仕様準拠 未実装/要補完タスク一覧

以下は `spec/` に定義された Nyx Protocol v1.0 の要件に対し、コードベースで未実装または不足している点を精査し、実装タスクとして整理したものです。各タスクは根拠となる仕様/ドキュメントと、主な対象ファイルを併記します。

---

### 1. Telemetry (OTLP) 強化: ハンドシェイク計測の不足
- [x] `nyx.handshake` スパンを実装（開始/完了、属性 `pq_mode`）
- [x] `setting_ids::PQ_MODE` と整合し最終モードをスパン属性へ反映（`nyx-stream`→`nyx-crypto` に伝播）
- [x] `nyx-telemetry` に `nyx.handshake` の捕捉テストを追加（in-memory/Exporter）

### 2. SETTINGS: Low Power Preference 広告・反映の未実装
- [x] `LOW_POWER_PREFERENCE` 設定項目を追加し `SettingsFrame` に統合
- [x] 受信 SETTINGS の値を `LowPowerManager` に反映（即時通知）
- [x] 送信側 SETTINGS の初期値を端末状態/ユーザ選好で決定（`NYX_LOW_POWER`）

### 3. Low Power 時の Keepalive/タイマ調整の未適用
- [x] Low Power 状態変化で `nyx-transport` の keepalive/idle timeout を動的変更（TCP fallback 経路）
- [x] 通常=15–30s、Low Power=60s（env で上書き可: `NYX_TCP_KEEPALIVE(_LP)`, `NYX_TCP_IDLE(_LP)`）
- [x] 低電力遷移の E2E テストを追加（WASM SETTINGS → HTTP 200 / フック発火）

### 4. Anti-Replay/0-RTT Telemetry の欠落
- [x] `nyx_replay_drop_total` / `nyx_early_data_accept_total` を追加
- [x] `AeadError::Replay` でリプレイドロップをカウント
- [x] 0-RTT 受理パスで受理件数をカウント（Handshake payload 受理時に計測し、テスト追加）

### 5. Multipath WRR v2: Loss 反映重みの未実装
- [x] `update_path_quality(path_id, rtt, loss_rate)` を追加（`update_path_with_quality`）
- [x] 重み計算に `(1 - loss_rate)` を適用（下限クランプ）
- [x] Loss 条件の分布テストを追加

### 6. SETTINGS の PQ モード整合性（v0.1 → v1.0 移行）
- [x] 交渉を `PQ_MODE` に統一、`PQ_SUPPORTED` から移行（`nyx-stream` は両対応に拡張）
- [x] WASM/CLI を含む設定の単一化（`NYX_PQ_MODE`/`NYX_LOW_POWER` 対応）
- [x] 関連テスト（設定/交渉）を更新

### 7. Plugin SETTINGS の搬送形式の簡略化見直し
- [x] PLUGIN_REQUIRED/OPTIONAL を CBOR 配列搬送に正式対応（拡張 SETTINGS: `0xFFFF` セクション）
- [x] 互換 API を維持（TLV 側は件数、拡張 CBOR 側は配列本体）し HTTP ゲートウェイで両対応
- [x] テストを新旧両対応に強化（`nyx-stream/tests/plugin_settings_ext.rs`, `nyx-daemon` 単体）

### 8. Handshake → Telemetry の属性整合
- [x] `nyx.handshake` スパンに `pq_mode` を付与
- [x] 可能なら `cid` を属性として付与（暫定: `unknown`）

### 9. cMix/Delay 設定の仕様デフォルト値バインド確認
- [x] 既定値が仕様（Batch=100, VDF=100ms）と一致するか検証（env で上書き可: `NYX_CMIX_BATCH`, `NYX_CMIX_VDF_MS`）
- [x] 初期化時に補正・テストで固定化

### 10. OTLP/依存バージョンの混在解消（安定化）
- [x] `opentelemetry` 系依存のバージョンを統一（0.29 系）
- [x] API 変更の影響を抑え、テレメトリ e2e テストを通過

### 11. 0-RTT 受理パスの公開/検証強化
- [ ] 受理パスの実装位置を明確化し堅牢性（再送/リプレイ/誤順序）を強化
- [x] Telemetry で受理件数を計測

---

#### 参照（抜粋）
- 仕様/設計: `spec/Nyx_Protocol_v1.0_Spec_EN.md`, `docs/en/Nyx_Protocol_v1.0_Spec.md`, `docs/WRR_SCHEDULER_V2.md`, `docs/LOW_POWER_MODE.md`, `docs/MOBILE_POWER_PUSH_INTEGRATION.md`
- 実装箇所（例）:
  - Telemetry: `nyx-telemetry/src/{lib.rs, otlp.rs, opentelemetry_integration.rs}`
  - Handshake/AEAD: `nyx-crypto/src/{noise.rs, aead.rs}`
  - SETTINGS/管理: `nyx-stream/src/{management.rs, settings.rs}`
  - Multipath/WRR: `nyx-stream/src/scheduler_v2.rs`
  - Low Power: `nyx-core/src/low_power.rs`, `nyx-transport/src/{lib.rs, tcp_fallback.rs}`

