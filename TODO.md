# Nyx プロトコル実装チェックリスト

> `spec### 1.2 ハイブリッドハンドシェイクの実装統合
**参照**:- [x] Capability Negotiatio### 1.4 Post-Compromise Recovery (PCR) フロー
**参照**: `spec/Nyx_Design_Document_EN.md` §5.2

- [x] 侵害検出トリガー定義
  - [x] `nyx-core/src/security.rs` に検出インターフェース
  - [x] 異常トラフィックパターン検出(プラグイン可能)
  - [x] 外部シグナル(管理 API 経由)受信
- [x] PCR 実行ロジック
  - [x] `nyx-crypto/src/pcr.rs` に鍵ローテーション実装
  - [x] Forward Secrecy 保証のための ephemeral 鍵再生成
  - [x] セッション再確立プロトコル
- [x] 監査ログ
  - [x] PCR イベント記録(タイムスタンプ、理由)
  - [x] `nyx-daemon` の audit log へ出力 最初の CRYPTO フレームに capability リスト埋め込み
  - [x] `nyx-stream/src/capability.rs::negotiate` 呼び出し
  - [x] 失敗時 CLOSE 0x07 発行(4 バイト unsupported ID 付き)ec/Nyx_Protocol_v1.0_Spec_EN.md` §3, §Hybrid Post-Quantum Handshake

- [x] ハンドシェイク状態マシンの実装
  - [x] `nyx-stream/src/handshake.rs` 新規作成
  - [x] クライアント初期化フロー(鍵ペア生成 → CRYPTO フレーム送信)
  - [x] サーバー応答フロー(カプセル化 → CRYPTO フレーム送信)
  - [x] 最終確認フレーム処理
  - [x] トラフィック鍵導出(HKDF-Expand with labels)
- [x] CRYPTO フレーム定義
  - [x] `nyx-stream/src/frame.rs` に CRYPTO フレームタイプ追加
  - [x] ペイロード構造(ハイブリッド公開鍵/暗号文)
  - [x] シリアライゼーション/デシリアライゼーション
- [x] アンチリプレイ保護
  - [x] 方向別ノンスウィンドウ(2^20 サイズ)実装
  - [x] `nyx-stream/src/replay_protection.rs` 作成
  - [x] ウィンドウ外/重複フレーム検出とリジェクト
  - [x] リキー時のウィンドウリセット処理の整合性確認（2025-10-01 更新）
>
> 凡例：`[ ]` 未着手、`[~]` 部分実装/進行中、`[x]` 完了

---

## 1. 暗号化とハンドシェイク

### 1.1 BIKE KEM サポート（PQ-Only モード）
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §Feature Differences, §5.3

- [ ] `nyx-crypto/src/bike.rs` モジュール作成
  - [ ] BIKE-L1 鍵生成実装（`keygen() -> (PublicKey, SecretKey)`）
  - [ ] カプセル化実装（`encapsulate(pk) -> (Ciphertext, SharedSecret)`）
  - [ ] デカプセル化実装（`decapsulate(sk, ct) -> SharedSecret`）
  - [ ] エラーハンドリング（不正な鍵長・暗号文サイズ検証）
- [ ] ハイブリッド構成への組み込み
  - [ ] `nyx-crypto/src/hybrid.rs` に BIKE モード追加
  - [ ] X25519 + BIKE 鍵結合ロジック
  - [ ] KDF による共有秘密導出（HKDF-SHA256）
- [ ] テストスイート
  - [ ] 単体テスト（`nyx-crypto/tests/bike.rs`）
  - [ ] ラウンドトリップ検証（encap/decap 一致確認）
  - [ ] 不正入力に対する堅牢性テスト
- [ ] CI/CD 統合
  - [ ] `Cargo.toml` に `bike` feature 追加
  - [ ] GitHub Actions でフィーチャーゲート付きビルド
  - [ ] ベンチマーク追加（`benches/bike.rs`）

### 1.2 ハイブリッドハンドシェイクの実装統合
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §3, §Hybrid Post-Quantum Handshake

- [x] ハンドシェイク状態マシンの実装
  - [x] `nyx-stream/src/handshake.rs` 新規作成
  - [x] クライアント初期化フロー（鍵ペア生成 → CRYPTO フレーム送信）
  - [x] サーバー応答フロー（カプセル化 → CRYPTO フレーム送信）
  - [x] 最終確認フレーム処理
  - [x] トラフィック鍵導出（HKDF-Expand with labels）
- [x] CRYPTO フレーム定義
  - [x] `nyx-stream/src/frame.rs` に CRYPTO フレームタイプ追加
  - [x] ペイロード構造（ハイブリッド公開鍵/暗号文）
  - [x] シリアライゼーション/デシリアライゼーション
- [x] アンチリプレイ保護
  - [x] 方向別ノンスウィンドウ（2^20 サイズ）実装
  - [x] `nyx-stream/src/replay_protection.rs` 作成
  - [x] ウィンドウ外/重複フレーム検出とリジェクト
  - [x] リキー時のウィンドウリセット処理
- [x] Capability Negotiation の統合
  - [x] 最初の CRYPTO フレームに capability リスト埋め込み
  - [x] `nyx-stream/src/capability.rs::negotiate` 呼び出し
  - [x] 失敗時 CLOSE 0x07 発行（4 バイト unsupported ID 付き）
- [x] セッションマネージャへの接続
  - [x] `nyx-daemon/src/session_manager.rs` から handshake 起動
  - [x] 成功時にトラフィック鍵を session state に格納
  - [x] IPC/gRPC 経由でステータス公開 → REST API (axum) 実装 (session_api.rs, 5 tests passed)

### 1.3 HPKE リキー統合
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §5.3, §10

- [x] リキースケジューラ実装
  - [x] `nyx-stream/src/rekey_scheduler.rs` 作成
  - [x] 1 GB データ転送または 10 分経過でトリガー
  - [x] HPKE Seal/Open による鍵更新メッセージ交換
  - [x] 新鍵への切り替えとアトミック更新
- [x] 鍵材料の安全な破棄
  - [x] 旧鍵の `zeroize` 実行確認
  - [x] メモリスクラブ検証テスト
- [x] テレメトリ連携
  - [ ] リキー回数カウンター（`nyx.stream.rekey.count`）
  - [ ] リキー失敗率メトリクス

### 1.4 Post-Compromise Recovery (PCR) フロー
**参照**: `spec/Nyx_Design_Document_EN.md` §5.2

- [ ] 侵害検出トリガー定義
  - [ ] `nyx-core/src/security.rs` に検出インターフェース
  - [ ] 異常トラフィックパターン検出（プラグイン可能）
  - [ ] 外部シグナル（管理 API 経由）受信
- [ ] PCR 実行ロジック
  - [ ] `nyx-crypto/src/pcr.rs` に鍵ローテーション実装
  - [ ] Forward Secrecy 保証のための ephemeral 鍵再生成
  - [ ] セッション再確立プロトコル
- [ ] 監査ログ
  - [ ] PCR イベント記録（タイムスタンプ、理由）
  - [ ] `nyx-daemon` の audit log へ出力

---

## 2. ストリームレイヤと Capability Negotiation

### 2.1 Capability Negotiation ハンドシェイク
**参照**: `spec/Capability_Negotiation_Policy_EN.md`

- [x] ネゴシエーションフローの実装
  - [x] `nyx-stream/src/capability.rs::negotiate` の呼び出し統合
  - [x] ハンドシェイク完了前に capability 一致確認
  - [x] 不一致時の CLOSE 0x07 フレーム生成と送信
- [x] エラー伝播
  - [x] `nyx-daemon` へのエラー通知
  - [x] クライアント SDK へのエラー詳細返却
- [x] テスト
  - [ ] 必須 capability 不足時の切断テスト
  - [ ] オプション capability の無視動作確認

### 2.2 Connection Manager 実装
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §1, §7

- [x] `nyx-daemon/src/connection_manager.rs` 実装
  - [x] コネクション ID 管理
  - [x] BBR輻輳制御アルゴリズム (cwnd, btlbw, rtprop tracking)
  - [x] RTT推定器 (EWMA with min/max tracking, RFC 6298)
  - [x] Token bucket レート制限
  - [x] 再送キュー管理
- [x] フロー制御統合
  - [x] 送信可否判定 (cwnd + rate limiter)
  - [x] ACK処理でのcwnd/bandwidth更新
  - [x] パケット送信記録
- [x] ACK/STREAM フレーム処理
  - [x] ACK フレーム受信時の再送キュー更新
  - [x] RTT サンプル更新とBBR状態更新
- [x] REST API 公開 (connection_api.rs, 5 tests passed)
  - [x] GET /api/v1/connections - コネクション一覧取得
  - [x] GET /api/v1/connections/:id - 詳細取得 (RTT、帯域幅、パケット統計)
  - [x] POST /api/v1/connections/:id/close - クローズ

### 2.3 Stream Manager 実装
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §4.1

- [x] `nyx-daemon/src/stream_manager.rs` 実装 (651行, 7 tests passed)
  - [x] ストリーム ID 割り当て（奇数/偶数分離）
    - [x] Client-initiated: 奇数 (1, 3, 5, ...)
    - [x] Server-initiated: 偶数 (2, 4, 6, ...)
  - [x] 双方向/単方向ストリーム管理
    - [x] StreamType::Bidirectional / Unidirectional
    - [x] 最大ストリーム数制限 (max_bidi_streams, max_uni_streams)
  - [x] ストリーム状態追跡（Open, HalfClosed, Closed）
    - [x] 状態遷移: Open → HalfClosed{Send/Recv} → Closed
    - [x] FINフレームでのhalf-close処理
- [x] 多重化処理
  - [x] フレーム振り分けロジック（stream ID ベース on_frame_received）
  - [x] バックプレッシャー処理（flow control window, recv buffer limit）
- [x] CLOSE フレーム処理
  - [x] ストリーム終了通知 (close_send/close_recv)
  - [x] リソース解放 (自動カウント減算)

### 2.4 Multipath スケジューリング統合
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §2

- [ ] スケジューラのランタイム統合
  - [ ] `nyx-stream/src/multipath_dataplane.rs::PathScheduler` を session に組み込み
  - [ ] パス選択ロジックの呼び出し（送信前）
  - [ ] パス別送信キュー管理
- [ ] パスヘルスメトリクス
  - [ ] RTT、ジッター、パケットロス測定
  - [ ] `PathMetrics` の定期更新
  - [ ] 劣化パスの自動無効化
- [ ] リオーダリングバッファ
  - [ ] `ReorderingBuffer` の初期化
  - [ ] シーケンス番号ベースの順序回復
  - [ ] タイムアウト処理

### 2.5 Extended Packet Format の End-to-End 実装
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §7

- [ ] 送信パス
  - [ ] トランスポート層送信前に `ExtendedPacketHeader::encode` 呼び出し
  - [ ] CID、PathID、Type+Flags、Length の正確な設定
  - [ ] ペイロード 1280 バイト境界パディング
- [ ] 受信パス
  - [ ] 受信パケットの `ExtendedPacketHeader::decode` 検証
  - [ ] 不正パケット（長さ超過、不正フラグ）の破棄
  - [ ] デコード後の上位層への引き渡し
- [ ] テスト
  - [ ] パケット境界条件テスト（最大長、最小長）
  - [ ] 破損パケットの拒否テスト

---

## 3. ミックスルーティングとカバートラフィック

### 3.1 Mix Layer のライブ統合
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §4, §5

- [ ] cMix Integration Manager の組み込み
  - [ ] `nyx-daemon` セッションパイプラインに `CmixIntegrationManager` 初期化
  - [ ] 非同期タスクとしてバッチ処理ループ起動
  - [ ] バッチサイズ・VDF 遅延の設定読み込み（`nyx.toml`）
- [ ] アダプティブカバートラフィック連携
  - [ ] `nyx-mix::AdaptiveCoverManager` からカバー率取得
  - [ ] `nyx-stream` スケジューラへのカバーパケット注入
  - [ ] 目標利用率 U ∈ [0.2, 0.6] の維持ロジック
- [ ] 設定ノブ
  - [ ] `nyx.toml` に `[mix]` セクション追加
  - [ ] `enabled`, `batch_size`, `vdf_delay_ms`, `target_utilization` パラメータ

### 3.2 LARMix++ フィードバックループ
**参照**: `spec/Nyx_Design_Document_EN.md` §4.2

- [ ] トランスポートプローブからの統計取得
  - [ ] `nyx-transport/src/path_validation.rs` からメトリクス取得
  - [ ] レイテンシ、パケットロス、帯域幅を `PathBuilder` に供給
- [ ] 動的ホップ数調整
  - [ ] `nyx-stream::multipath_dataplane::adjust_hop_count` ロジックの有効化
  - [ ] ネットワーク状態変化時のホップ数増減
- [ ] パス劣化検出イベント
  - [ ] しきい値超過時のイベント発行
  - [ ] 代替パスへの自動フェイルオーバー

### 3.3 RSA Accumulator Proofs 配布
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §4

- [ ] Proof 生成
  - [ ] `nyx-mix::accumulator` でバッチごとに proof 計算
  - [ ] Proof の署名と timestamp 付与
- [ ] Proof 公開エンドポイント
  - [ ] `nyx-daemon` に `/proofs` エンドポイント追加（gRPC/HTTP）
  - [ ] DHT トピックへの Proof 配信
- [ ] 検証ロジック
  - [ ] クライアントによる proof 検証
  - [ ] 検証結果のメトリクス記録

---

## 4. トランスポートと NAT トラバーサル

### 4.1 QUIC Datagram 実装
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §6.1

- [ ] QUIC スタック選定と統合
  - [ ] `quinn` クレート統合（または自作実装の選択）
  - [ ] Datagram 拡張の有効化
  - [ ] `nyx-transport/src/quic.rs` の置き換え
- [ ] パケット暗号化
  - [ ] Initial/Handshake/Application 暗号化レベル実装
  - [ ] 鍵更新処理
  - [ ] パケット番号暗号化
- [ ] ストリーム/Datagram 多重化
  - [ ] QUIC ストリームと Datagram の同時利用
  - [ ] フレームタイプ別の処理振り分け
- [ ] 輻輳制御
  - [ ] BBR アルゴリズムの適用（または CUBIC）
  - [ ] ミックスネットワーク特性への最適化
- [ ] パス移行
  - [ ] PATH_CHALLENGE/PATH_RESPONSE フレーム実装
  - [ ] マルチパス対応のための拡張

### 4.2 ICE Lite 候補収集
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §6.2

- [ ] STUN Binding 実装
  - [ ] STUN メッセージ構築とパース
  - [ ] UDP ソケット経由での STUN リクエスト送信
  - [ ] Server Reflexive アドレスの取得
- [ ] TURN Allocation 実装
  - [ ] TURN Allocate Request/Response
  - [ ] Relay アドレスの取得と管理
  - [ ] TURN Channel Binding
- [ ] 候補ペア生成
  - [ ] ローカル/リモート候補の総当たりペア化
  - [ ] 優先度計算（RFC 5245 準拠）
- [ ] 並列接続性チェック
  - [ ] 候補ペアごとの STUN Connectivity Check
  - [ ] 成功ペアの RTT 記録
  - [ ] ランキングとベストパス選択

### 4.3 Teredo / IPv6 デュアルスタック実装
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §6.3

- [ ] Teredo 検出
  - [ ] システムの Teredo アダプタ検出
  - [ ] Teredo サーバーアドレス取得
- [ ] トンネル確立
  - [ ] Teredo パケットのカプセル化/デカプセル化
  - [ ] IPv6 over IPv4 UDP 送受信
- [ ] フォールバック選択
  - [ ] IPv6 優先、利用不可時は IPv4 へフォールバック
  - [ ] RFC 6724 アドレス選択アルゴリズム

### 4.4 パス検証とプロービング
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §6.1, §6.2

- [ ] アクティブプローブ実装
  - [ ] `nyx-control/src/probe.rs` にプローブロジック追加
  - [ ] 定期的な Ping/Pong メッセージ送信
  - [ ] RTT、パケットロス測定
- [ ] メトリクスフィード
  - [ ] プローブ結果を `PathBuilder` へ供給
  - [ ] マルチパススケジューラへのメトリクス反映
- [ ] エンドポイント検証
  - [ ] `nyx-transport/src/path_validation.rs` 実装
  - [ ] 到達性確認と無効パスの除外

---

## 5. デーモンとコントロールプレーン

### 5.1 gRPC コントロール API
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §Daemon

- [ ] Protobuf 定義
  - [ ] `nyx-daemon/proto/` に `.proto` ファイル作成
  - [ ] サービス定義（Session, Config, Metrics）
  - [ ] メッセージ型定義（Request/Response）
- [ ] コード生成
  - [ ] `tonic` または `prost` を使用した Rust コード生成
  - [ ] ビルドスクリプト（`build.rs`）設定
- [ ] サーバー実装
  - [ ] `nyx-daemon/src/grpc_server.rs` 作成
  - [ ] 各 RPC メソッドの実装
  - [ ] TLS/mTLS サポート（オプション）
- [ ] クライアント統合
  - [ ] `nyx-sdk/src/grpc_client.rs` 作成
  - [ ] JSON IPC との互換性ブリッジ
  - [ ] 段階的移行計画

### 5.2 セッション/パスオーケストレーションモジュール
**参照**: 空ファイル群の実装

#### 5.2.1 Session Manager
- [ ] `nyx-daemon/src/session_manager.rs` 実装
  - [ ] セッション状態管理（Map<CID, Session>）
  - [ ] ハンドシェイク完了後の登録
  - [ ] フレームルーティング（CID ベース）
  - [ ] Capability Negotiation の管理

#### 5.2.2 Stream Manager
- [ ] `nyx-daemon/src/stream_manager.rs` 実装
  - [ ] ストリーム多重化
  - [ ] フロー制御統合
  - [ ] バックプレッシャー処理

#### 5.2.3 Pure Rust DHT
- [ ] `nyx-daemon/src/pure_rust_dht.rs` 実装
  - [ ] Kademlia ルーティングテーブル
  - [ ] FIND_NODE/FIND_VALUE クエリ
  - [ ] UDP ベースの RPC
  - [ ] ノード発見とブートストラップ

#### 5.2.4 Pure Rust P2P
- [ ] `nyx-daemon/src/pure_rust_p2p.rs` 実装
  - [ ] ピア接続管理
  - [ ] メッセージフレーミング（length-prefixed）
  - [ ] ピア発見プロトコル

#### 5.2.5 Push Notification Relay
- [ ] `nyx-daemon/src/push.rs` 実装
  - [ ] FCM 統合（HTTP v1 API）
  - [ ] APNS 統合（HTTP/2 API）
  - [ ] WebPush 統合（VAPID）
  - [ ] 資格情報管理とリトライロジック
  - [ ] `nyx-core::push::PushProvider` trait 実装

#### 5.2.6 Proto 定義管理
- [ ] `nyx-daemon/src/proto.rs` 実装
  - [ ] Protobuf メッセージの再エクスポート
  - [ ] 内部型との変換ロジック

### 5.3 Path Builder の統合強化
**参照**: `nyx-daemon/src/path_builder.rs`

- [ ] ライブメトリクス更新
  - [ ] トランスポートプローブからの統計取得
  - [ ] `nyx-mix` からの経路品質フィード
  - [ ] 定期的なメトリクス更新タスク
- [ ] 動的経路再構成
  - [ ] パス劣化検出時の自動再構築
  - [ ] 負荷分散ロジックの改善

### 5.4 設定同期と分散制御
**参照**: `spec/Nyx_Design_Document_EN.md` §9.3

- [ ] ネットワーク化された DHT
  - [ ] `nyx-control/src/dht.rs` に UDP 送受信追加
  - [ ] Kademlia プロトコル実装
  - [ ] ルーティングテーブルの永続化
- [ ] 設定ゴシップ
  - [ ] 設定変更の伝播メカニズム
  - [ ] バージョン管理と競合解決
- [ ] ランデブーサービス
  - [ ] `nyx-control/src/rendezvous.rs` のネットワーク統合
  - [ ] 登録/検索 API の公開

---

## 6. テレメトリとオブザーバビリティ

### 6.1 OTLP Exporter 実装
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §9

- [ ] `nyx-telemetry/src/otlp.rs` 実装
  - [ ] OpenTelemetry SDK 統合
  - [ ] OTLP gRPC/HTTP エクスポータ設定
  - [ ] スパン生成とバッチング
  - [ ] グレースフルシャットダウン
- [ ] 設定
  - [ ] `nyx.toml` に OTLP エンドポイント設定
  - [ ] サンプリングレート設定
- [ ] テスト
  - [ ] モックコレクター使用のユニットテスト
  - [ ] スパン生成の検証

### 6.2 ストリームレイヤのテレメトリ充実化
**参照**: `nyx-stream/src/telemetry_schema.rs`

- [ ] クリティカルパスの計装
  - [ ] フレーム送受信時のスパン生成
  - [ ] マルチパス決定時の属性記録
  - [ ] ハンドシェイク各段階のスパン
- [ ] 呼び出し元の統合
  - [ ] `nyx-stream` の各モジュールから `TelemetryContext` 呼び出し
  - [ ] スパン階層の構築（親子関係）
- [ ] OTLP/Prometheus へのエクスポート
  - [ ] スパンデータの OTLP 送信
  - [ ] メトリクスの Prometheus カウンター登録

### 6.3 Prometheus 統合の拡充
**参照**: `nyx-daemon/src/prometheus_exporter.rs`

- [ ] 追加メトリクス定義
  - [ ] ハンドシェイク成功/失敗カウンター
  - [ ] パス品質ゲージ（RTT、パケットロス）
  - [ ] カバートラフィック利用率ゲージ
  - [ ] cMix バッチ処理回数/遅延ヒストグラム
- [ ] メトリクス登録
  - [ ] `nyx-telemetry/src/metrics.rs` へのレジストリ追加
  - [ ] ラベル設計（path_id, session_id など）
- [ ] エクスポータ起動確認
  - [ ] `NYX_PROMETHEUS_ADDR` 環境変数サポート
  - [ ] `/metrics` エンドポイント検証

---

## 7. モバイルとローパワーモード

### 7.1 Screen-off Detector の実装
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §6.6

- [ ] `nyx-daemon/src/screen_off_detection.rs` 修正
  - [ ] コンパイルエラーの修正（型不一致、未定義シンボル）
  - [ ] スクリーン状態追跡ロジック
  - [ ] オフ比率計算（`screen_off_ratio`）
  - [ ] パワーステート決定（Active, Background, etc.）
- [ ] Low Power Bridge との統合
  - [ ] `nyx-daemon/src/low_power.rs` からの状態更新呼び出し
  - [ ] イベントシステムへの通知
- [ ] 設定とイベント
  - [ ] `nyx.toml` に低電力設定追加
  - [ ] `power` イベント発行とクライアント通知

### 7.2 プッシュ通知リレー実装
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §6.6

- [ ] `nyx-daemon/src/push.rs` 実装
  - [ ] `nyx-core::push::PushProvider` trait の具体実装
  - [ ] FCM HTTP v1 API クライアント（`reqwest` 使用）
  - [ ] APNS HTTP/2 API クライアント（`h2` 使用）
  - [ ] WebPush VAPID 署名とリクエスト構築
- [ ] 資格情報管理
  - [ ] サービスアカウント JSON 読み込み（FCM）
  - [ ] APNS 証明書/トークン管理
  - [ ] VAPID 鍵ペア生成と保存
- [ ] リトライと信頼性
  - [ ] 失敗時の指数バックオフ
  - [ ] デッドレターキューまたはログ記録
- [ ] 設定公開
  - [ ] `nyx.toml` に `[push]` セクション
  - [ ] 認証情報パス、タイムアウト設定

### 7.3 ローパワーランタイムテレメトリ
**参照**: `nyx-daemon/src/low_power.rs`

- [ ] テスト追加
  - [ ] モックプラットフォーム状態を使用した単体テスト
  - [ ] パワーステート遷移の検証
- [ ] カバートラフィック自動調整
  - [ ] パワーイベント受信時に `AdaptiveCoverManager` へフィード
  - [ ] cover_ratio の動的変更
  - [ ] 目標利用率レンジの調整（Background 時は [0.1, 0.3]）
- [ ] しきい値設定公開
  - [ ] `nyx.toml` で `screen_off_threshold`, `battery_threshold` 等を設定可能に

---

## 8. コンプライアンス、Capability、ポリシー

### 8.1 デーモン起動時のコンプライアンスレベル検出
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §10

- [ ] 起動フロー統合
  - [ ] `nyx-daemon/src/main.rs` で `nyx-core::compliance::determine_compliance_level` 呼び出し
  - [ ] 検出レベル（Core, Plus, Full）のログ出力
  - [ ] テレメトリへの記録
- [ ] コントロール API 公開
  - [ ] gRPC/IPC 経由でコンプライアンスレポート取得
  - [ ] CLI サブコマンド（`nyx-cli compliance`）追加

### 8.2 Capability Negotiation 失敗コードの伝播
**参照**: `spec/Capability_Negotiation_Policy_EN.md`

- [ ] CLOSE 0x07 フレーム生成確認
  - [ ] `nyx-stream/src/management.rs::build_close_unsupported_cap` 呼び出しパス確認
  - [ ] 4 バイト unsupported ID のシリアライゼーション
- [ ] クライアント SDK へのエラー返却
  - [ ] `nyx-sdk/src/error.rs` に `UnsupportedCapability` エラー追加
  - [ ] エラーメッセージに capability ID 含める

### 8.3 ポリシー駆動のプラグイン権限
**参照**: `spec/Nyx_Protocol_v1.0_Spec_EN.md` §1

- [ ] Capability 検証ゲート
  - [ ] `nyx-stream/src/plugin_dispatch.rs` で negotiated capabilities チェック
  - [ ] 未許可 capability のプラグイン呼び出し拒否
- [ ] サンドボックス設定連動
  - [ ] Capability フラグに基づくサンドボックスポリシー選択
  - [ ] `nyx-stream/src/plugin_sandbox.rs` の強化

---

## 9. テストと検証

### 9.1 エンドツーエンド統合テスト
**参照**: `spec/testing/*.md`

- [ ] テストハーネス構築
  - [ ] `tests/integration/` ディレクトリ作成
  - [ ] マルチノードシミュレータ（デーモン複数起動）
  - [ ] ハンドシェイク完全フロー検証
  - [ ] マルチパスデータ転送テスト
  - [ ] カバートラフィック率測定
- [ ] CI 統合
  - [ ] GitHub Actions ワークフローに統合テスト追加
  - [ ] `cargo nextest` 導入検討
  - [ ] 並列実行とタイムアウト設定

### 9.2 形式手法モデルとの同期
**参照**: `formal/` ディレクトリ

- [ ] CI フック追加
  - [ ] `formal/run_model_checking.py` を CI で実行
  - [ ] TLC チェッカーの成功/失敗を CI ステータスに反映
- [ ] 不変条件の同期
  - [ ] コード変更時の TLA+ 仕様更新プロセス確立
  - [ ] 不変条件違反時のアラート

### 9.3 ファズおよびプロパティテストカバレッジ
**参照**: `fuzz/` ディレクトリ

- [ ] 新規ファズターゲット追加
  - [ ] `fuzz_targets/extended_packet.rs`（パケットパース）
  - [ ] `fuzz_targets/capability_negotiation.rs`（CBOR デコード）
  - [ ] `fuzz_targets/ice_candidate.rs`（ICE 候補パース）
  - [ ] `fuzz_targets/quic_packet.rs`（QUIC パケットデコード）
- [ ] CI でのファズ実行
  - [ ] OSS-Fuzz 統合または GitHub Actions での定期実行
  - [ ] クラッシュ時の自動 Issue 作成

---

## 10. ツーリング、ドキュメント、パッケージング

### 10.1 設定サーフェス拡張
**参照**: `nyx.toml`, `docs/configuration.md`

- [ ] `nyx.toml` スキーマ拡張
  - [ ] `[multipath]` セクション（`max_paths`, `min_path_quality`）
  - [ ] `[crypto]` セクション（`pq_mode`, `enable_bike`, `enable_kyber`）
  - [ ] `[telemetry]` セクション（`otlp_endpoint`, `otlp_sampling_rate`）
  - [ ] `[mix]` セクション（`cmix_enabled`, `batch_size`, `vdf_delay_ms`）
- [ ] ドキュメント更新
  - [ ] `docs/configuration.md` に新規設定項目追加
  - [ ] サンプル設定ファイル（`examples/full_config.toml`）作成
- [ ] CLI サポート
  - [ ] `nyx-cli config validate` サブコマンド追加
  - [ ] 設定値のスキーマ検証

### 10.2 ドキュメント整合性維持
**参照**: `docs/` ディレクトリ

- [ ] API ドキュメント更新
  - [ ] `docs/api.md` に gRPC エンドポイント追記
  - [ ] JSON IPC から gRPC への移行ガイド
- [ ] 仕様ドキュメント同期
  - [ ] `docs/specs.md` の更新（実装済み機能のマーク）
  - [ ] `spec/` との差分チェック自動化（CI スクリプト）
- [ ] アーキテクチャ図更新
  - [ ] `docs/architecture.md` のコンポーネント図刷新
  - [ ] マルチパス、cMix、OTLP フローの追加

### 10.3 Helm Chart / デプロイフック
**参照**: `charts/nyx`

- [ ] Values 拡張
  - [ ] `values.yaml` に OTLP エンドポイント設定追加
  - [ ] gRPC ポート設定
  - [ ] 追加シークレット（FCM, APNS 資格情報）
- [ ] ConfigMap 更新
  - [ ] `nyx.toml` の ConfigMap テンプレート更新
  - [ ] 環境変数マッピング
- [ ] サービス定義
  - [ ] gRPC サービス用の Service リソース追加
  - [ ] ヘルスチェックエンドポイント設定

---

## 実装優先順位と推奨シーケンス

### フェーズ 1: ネットワークスタック解放（最優先）
1. [ ] QUIC Datagram 実装（§4.1）
2. [ ] ICE Lite 候補収集（§4.2）
3. [ ] Session/Stream Manager 実装（§2.2, §2.3）
4. [ ] Extended Packet Format 統合（§2.5）

**目的**: エンドツーエンドのデータ転送とテスト能力の確保

### フェーズ 2: セキュアチャネル確立
5. [ ] ハイブリッドハンドシェイク統合（§1.2）
6. [ ] Capability Negotiation ハンドシェイク（§2.1）
7. [ ] アンチリプレイ保護（§1.2）
8. [ ] HPKE リキー統合（§1.3）

**目的**: 仕様準拠のセキュアな通信チャネル実現

### フェーズ 3: 匿名性とパフォーマンス
9. [ ] Mix Layer ライブ統合（§3.1）
10. [ ] cMix Integration Manager（§3.1）
11. [ ] アダプティブカバートラフィック連携（§3.1）
12. [ ] Multipath スケジューリング統合（§2.4）
13. [ ] LARMix++ フィードバックループ（§3.2）

**目的**: プライバシー保護とネットワーク性能目標の達成

### フェーズ 4: コントロールプレーン完成
14. [ ] gRPC コントロール API（§5.1）
15. [ ] Pure Rust DHT（§5.2.3）
16. [ ] 設定同期と分散制御（§5.4）
17. [ ] OTLP Exporter（§6.1）
18. [ ] テレメトリ充実化（§6.2, §6.3）

**目的**: ユーザー/オペレーター向け機能公開と運用性向上

### フェーズ 5: モバイル・コンプライアンス
19. [ ] Screen-off Detector（§7.1）
20. [ ] プッシュ通知リレー（§7.2）
21. [ ] ローパワーテレメトリ（§7.3）
22. [ ] コンプライアンスレベル検出（§8.1）
23. [ ] ポリシー駆動プラグイン権限（§8.3）

**目的**: 残存仕様ギャップの解消とモバイル対応完了

### 継続的活動
- [ ] エンドツーエンド統合テスト拡充（§9.1）
- [ ] ファズターゲット追加（§9.3）
- [ ] ドキュメント整合性維持（§10.2）
- [ ] CI/CD パイプライン強化

---

## 進捗管理

**更新頻度**: 週次レビュー
**ステータス更新**: マイルストーン完了時に `[ ]` → `[x]` へチェック
**レビュアー**: プロジェクトメンテナー
**次回更新予定**: 2025-10-08
