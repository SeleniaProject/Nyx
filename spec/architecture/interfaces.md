# docs/architecture/interfaces.md

> **遵守バッジ** : :no_entry: 実装コード非出力 / :no_entry_sign: C/C++依存禁止

## 目次
- [目的と範囲](#目的と範囲)
- [バージョニングポリシー](#バージョニングポリシー)
- [内部インターフェース](#内部インターフェース)
- [外部インターフェース](#外部インターフェース)
- [エラー処理とリトライ](#エラー処理とリトライ)
- [互換性管理](#互換性管理)
- [タイムアウト・レート制限](#タイムアウトレート制限)
- [関連ドキュメント](#関連ドキュメント)

## 目的と範囲
Nyxプロジェクトにおける主要内部/外部APIを言語非依存の抽象スキーマで定義する。実装はRust/Go等で行うが、本書ではコードは記載しない。関連アーキテクチャは[overview.md](./overview.md)を参照。

## バージョニングポリシー
- **MAJOR.MINOR.PATCH**形式を採用。破壊的変更はMAJORを更新。
- バージョンネゴシエーションは初回ハンドシェイクで実施。
- API廃止は12ヶ月前に通知し、`deprecation_date`メタデータを付与。

## 内部インターフェース
### Stream Layer \u2192 Mix Layer
```
Interface: ROUTE_PLAN.REQUEST
Fields:
  - session_id: UUID
  - qos_profile: ENUM{INTERACTIVE, BULK}
  - anonymity_level: ENUM{STANDARD, HIGH, EXTREME}
Response: ROUTE_PLAN.REPLY
  - route_id: UUID
  - node_sequence: LIST<NodeRef>
  - ttl_seconds: Integer
```
- **契約**: `node_sequence`は3ノード以上。TTLが切れる前に再評価実施。

### Mix Layer \u2192 Obfuscation Layer
```
Message: ROUTED_PACKET
  - route_id
  - hop_index
  - encrypted_payload
  - padding_size
```
- `padding_size`により固定長化を維持。

### Control Plane Broadcast
```
Event: POLICY_UPDATE
  - policy_id
  - version
  - payload_hash
  - effective_at (timestamp)
```
- 受信側は幂等適用。認証はmTLS + JWT。

## 外部インターフェース
### Client Handshake API
```
Endpoint: POST /nyx/v1/session
Headers:
  - Accept-Version: >=1.0
Body:
  - client_capabilities: LIST<String>
  - pq_support: Boolean
  - locale: ISO-639-1
Response:
  - session_token: String
  - stream_params: Map<String, Any>
  - expires_at: Timestamp
```
- **セキュリティ**: mTLS + OIDCトークン。詳細は[security/auth.md](../security/auth.md)。

### Telemetry Export API
```
Endpoint: PUT /telemetry/v1/metrics
Body:
  - session_id
  - metric_bundle: LIST<Metric>
Metric:
  - name: String
  - value: Number
  - unit: String
  - labels: Map<String,String>
```
- **制約**: 1リクエスト当たり1000メトリクスまで。

### Admin Audit Query
```
Endpoint: GET /governance/v1/audit-events
Query:
  - from: Timestamp
  - to: Timestamp
  - actor: Optional<String>
  - severity: Optional<ENUM{INFO,WARN,CRITICAL}>
Response:
  - events: LIST<AuditEvent>
AuditEvent:
  - event_id: UUID
  - actor
  - action
  - resource
  - outcome
```
- **制御**: RBAC/ABACを強制。

## エラー処理とリトライ
| エラーコード | 説明 | リトライポリシー | 通知 |
|--------------|------|------------------|------|
| `ERR-SESSION-409` | セッション重複 | クライアント側でidempotency key更新 | クライアント通知 |
| `ERR-ROUTE-503` | 経路利用不可 | 5s間隔で3回リトライ | SREへPagerDuty |
| `ERR-AUDIT-403` | アクセス拒否 | リトライ不可 | セキュリティ監査ログ |
| `ERR-PQ-426` | PQ非互換 | フォールバックルートを提示 | ADR検討 |

## 互換性管理
- 互換性マトリクスは`compatibility.yaml`（別途運用）で管理。
- ドキュメントでは[notes/decision-log.md](../notes/decision-log.md)に破壊的変更を記録。
- バージョン切替の移行期間は最低3リリース分。

## タイムアウト・レート制限
| API | タイムアウト | レート制限 | 備考 |
|-----|-------------|-----------|------|
| セッション確立 | 3秒 | 30req/min/クライアント | Bot攻撃を防止 |
| テレメトリ | 5秒 | 100req/min/ノード | バックプレッシャ対応 |
| 監査照会 | 10秒 | 10req/min/アクター | 監査ログ保護 |

## 拡張補遺: インターフェース適合性シナリオ集

この補遺では、主要内部・外部APIが仕様どおりに動作するかを定量評価するための50シナリオを定義する。各シナリオは対象API、バージョン、入力、期待レスポンス、契約要件、エラー制御、互換性、監査ポイント、検証方法、レート制限、フォローアップの11項目で構成され、CI契約テストおよび運用受入での利用を想定している。

#### シナリオ IF-001: セッション確立API v1.2 正常系
- **対象 IF-001-01**: `POST /nyx/v1/session`
- **バージョン IF-001-02**: Accept-Version=1.2
- **入力 IF-001-03**: 標準能力セット + PQ対応=true
- **期待 IF-001-04**: `session_token`発行、`stream_params`にMultipath=false
- **契約 IF-001-05**: レスポンスTTL≥3600秒
- **エラー制御 IF-001-06**: None
- **互換性 IF-001-07**: 旧SDK(v1.0)互換
- **監査 IF-001-08**: Auditログに`SESSION_CREATE`
- **検証 IF-001-09**: Integrationテスト ケース`session_happy_path`
- **レート IF-001-10**: 30req/min以下で検証
- **フォロー IF-001-11**: `testing/integration-tests.md`更新

#### シナリオ IF-002: セッション確立 PQ非対応フォールバック
- **対象 IF-002-01**: `POST /nyx/v1/session`
- **バージョン IF-002-02**: Accept-Version=1.1
- **入力 IF-002-03**: pq_support=false
- **期待 IF-002-04**: `session_token`と`pq_fallback=true`
- **契約 IF-002-05**: Fallback時`ERR-PQ-426`未発生
- **エラー制御 IF-002-06**: Warningレスポンスヘッダ
- **互換性 IF-002-07**: PQ未対応端末
- **監査 IF-002-08**: Policyログに`fallback_applied`
- **検証 IF-002-09**: Regressionテスト
- **レート IF-002-10**: 制限未達
- **フォロー IF-002-11**: ADRに決定記録

#### シナリオ IF-003: セッション確立 エラー409検証
- **対象 IF-003-01**: `POST /nyx/v1/session`
- **バージョン IF-003-02**: 1.0
- **入力 IF-003-03**: 冪等キー重複
- **期待 IF-003-04**: `ERR-SESSION-409`
- **契約 IF-003-05**: Retry-Afterヘッダ = 0
- **エラー制御 IF-003-06**: クライアント通知
- **互換性 IF-003-07**: 旧SDK
- **監査 IF-003-08**: Auditに`duplicate_session`
- **検証 IF-003-09**: 契約テスト
- **レート IF-003-10**: 制限内
- **フォロー IF-003-11**: ドキュメント更新

#### シナリオ IF-004: ROUTE_PLAN.REQUEST ミニマムパラメータ
- **対象 IF-004-01**: Stream→Mix RPC
- **バージョン IF-004-02**: 2.0
- **入力 IF-004-03**: qos_profile=INTERACTIVE, anonymity_level=STANDARD
- **期待 IF-004-04**: `node_sequence`長>=3
- **契約 IF-004-05**: TTL≥120秒
- **エラー制御 IF-004-06**: エラー0
- **互換性 IF-004-07**: Mix v1互換
- **監査 IF-004-08**: Telemetryタグ`route_plan`
- **検証 IF-004-09**: gRPC契約テスト
- **レート IF-004-10**: 500req/min以内
- **フォロー IF-004-11**: `architecture/dataflow.md`参照更新

#### シナリオ IF-005: ROUTE_PLAN.REPLY 最大匿名度
- **対象 IF-005-01**: Stream→Mix RPC
- **バージョン IF-005-02**: 2.0
- **入力 IF-005-03**: anonymity_level=EXTREME
- **期待 IF-005-04**: node_sequence>=5, TTL≥60
- **契約 IF-005-05**: 追加属性`mix_set_hash`
- **エラー制御 IF-005-06**: 503時再試行
- **互換性 IF-005-07**: Mix v2のみ
- **監査 IF-005-08**: Controlに匿名性強度ログ
- **検証 IF-005-09**: Integration
- **レート IF-005-10**: 200req/min
- **フォロー IF-005-11**: SLO調整

#### シナリオ IF-006: ROUTED_PACKET padding検証
- **対象 IF-006-01**: Mix→Obfuscation
- **バージョン IF-006-02**: 1.4
- **入力 IF-006-03**: padding_size=128
- **期待 IF-006-04**: Obfが固定長化
- **契約 IF-006-05**: padding_size<=256
- **エラー制御 IF-006-06**: 違反時ERR-OBF-422
- **互換性 IF-006-07**: Obf v1.3+
- **監査 IF-006-08**: Telemetry `padding_applied`
- **検証 IF-006-09**: 単体テスト
- **レート IF-006-10**: 1000pkt/s
- **フォロー IF-006-11**: Spec追記

#### シナリオ IF-007: POLICY_UPDATE 効力予約
- **対象 IF-007-01**: Control Broadcast
- **バージョン IF-007-02**: 1.5
- **入力 IF-007-03**: effective_at未来時刻
- **期待 IF-007-04**: 受信側が待機
- **契約 IF-007-05**: effective_at≥now+60s
- **エラー制御 IF-007-06**: 早適用禁止
- **互換性 IF-007-07**: 全層
- **監査 IF-007-08**: ガバナンスログ
- **検証 IF-007-09**: シミュレーション
- **レート IF-007-10**: 60event/min
- **フォロー IF-007-11**: Runbook更新

#### シナリオ IF-008: POLICY_UPDATE 冪等再送
- **対象 IF-008-01**: Control Broadcast
- **バージョン IF-008-02**: 1.4
- **入力 IF-008-03**: 同一payload再送
- **期待 IF-008-04**: 重複適用無
- **契約 IF-008-05**: payload_hash比較
- **エラー制御 IF-008-06**: Warning
- **互換性 IF-008-07**: 全層
- **監査 IF-008-08**: Duplicate検出
- **検証 IF-008-09**: 契約テスト
- **レート IF-008-10**: 120event/min
- **フォロー IF-008-11**: `docs/configuration.md`更新

#### シナリオ IF-009: Telemetry Export バルク投入
- **対象 IF-009-01**: `PUT /telemetry/v1/metrics`
- **バージョン IF-009-02**: 1.3
- **入力 IF-009-03**: metric_bundle=1000件
- **期待 IF-009-04**: バリデーションOK
- **契約 IF-009-05**: 1リクエスト上限遵守
- **エラー制御 IF-009-06**: 413時の分割ガイド
- **互換性 IF-009-07**: Telemetry v1
- **監査 IF-009-08**: Observabilityログ
- **検証 IF-009-09**: パフォーマンステスト
- **レート IF-009-10**: 100req/min
- **フォロー IF-009-11**: メトリクスダッシュボード更新

#### シナリオ IF-010: Telemetry Export ラベル制限
- **対象 IF-010-01**: `PUT /telemetry/v1/metrics`
- **バージョン IF-010-02**: 1.3
- **入力 IF-010-03**: labels 50個
- **期待 IF-010-04**: ERR-METRIC-422
- **契約 IF-010-05**: label<=30
- **エラー制御 IF-010-06**: 詳細メッセージ
- **互換性 IF-010-07**: Telemetry v1
- **監査 IF-010-08**: Rateログ
- **検証 IF-010-09**: Validationテスト
- **レート IF-010-10**: 制限未到達
- **フォロー IF-010-11**: SDKバリデーション更新

#### シナリオ IF-011: Admin Audit Query ページング
- **対象 IF-011-01**: `GET /governance/v1/audit-events`
- **バージョン IF-011-02**: 1.0
- **入力 IF-011-03**: from/to + limit=500
- **期待 IF-011-04**: events<=500, next_token付与
- **契約 IF-011-05**: ページング必須
- **エラー制御 IF-011-06**: 無
- **互換性 IF-011-07**: GovPortal v1
- **監査 IF-011-08**: Queryログ
- **検証 IF-011-09**: APIテスト
- **レート IF-011-10**: 5req/min
- **フォロー IF-011-11**: Runbook記載

#### シナリオ IF-012: Admin Audit Query 認可拒否
- **対象 IF-012-01**: `GET /governance/v1/audit-events`
- **バージョン IF-012-02**: 1.0
- **入力 IF-012-03**: RBAC権限不足
- **期待 IF-012-04**: `ERR-AUDIT-403`
- **契約 IF-012-05**: Auditイベント生成
- **エラー制御 IF-012-06**: Retry禁止
- **互換性 IF-012-07**: GovPortal v1
- **監査 IF-012-08**: 拒否ログ
- **検証 IF-012-09**: 負荷テスト
- **レート IF-012-10**: 制限未満
- **フォロー IF-012-11**: セキュリティレビュー

#### シナリオ IF-013: Stream Frame Error通知契約
- **対象 IF-013-01**: Stream→Clientエラー
- **バージョン IF-013-02**: 2.1
- **入力 IF-013-03**: Invalid Frame
- **期待 IF-013-04**: `FRAME_ERROR`イベント
- **契約 IF-013-05**: `reason_code`必須
- **エラー制御 IF-013-06**: 再送停止
- **互換性 IF-013-07**: SDK v2
- **監査 IF-013-08**: Telemetry
- **検証 IF-013-09**: 単体
- **レート IF-013-10**: 100err/min
- **フォロー IF-013-11**: SDKリリースノート

#### シナリオ IF-014: Stream Frame 多言語互換
- **対象 IF-014-01**: Streamフレーム
- **バージョン IF-014-02**: 2.1
- **入力 IF-014-03**: locale=ja
- **期待 IF-014-04**: メタデータUTF-8
- **契約 IF-014-05**: localeヘッダ必須
- **エラー制御 IF-014-06**: InvalidLocale時422
- **互換性 IF-014-07**: SDK多言語
- **監査 IF-014-08**: ロケールタグ
- **検証 IF-014-09**: i18nテスト
- **レート IF-014-10**: 通常
- **フォロー IF-014-11**: ドキュメント翻訳

#### シナリオ IF-015: Obfuscation Controlコマンド契約
- **対象 IF-015-01**: Control→Obfuscation RPC
- **バージョン IF-015-02**: 3.0
- **入力 IF-015-03**: `SetCoverProfile`
- **期待 IF-015-04**: ACK + ApplyTime
- **契約 IF-015-05**: ApplyTime≤120s
- **エラー制御 IF-015-06**: Timeout503
- **互換性 IF-015-07**: Obf v2
- **監査 IF-015-08**: AuditEvent
- **検証 IF-015-09**: Integration
- **レート IF-015-10**: 20cmd/min
- **フォロー IF-015-11**: Runbook

#### シナリオ IF-016: Obfuscation Control ロールバック
- **対象 IF-016-01**: Control→Obfuscation
- **バージョン IF-016-02**: 3.0
- **入力 IF-016-03**: rollback=true
- **期待 IF-016-04**: 旧設定再適用
- **契約 IF-016-05**: 履歴保持>=5
- **エラー制御 IF-016-06**: 失敗時ERR-CTRL-502
- **互換性 IF-016-07**: v2
- **監査 IF-016-08**: Rollbackイベント
- **検証 IF-016-09**: 統合テスト
- **レート IF-016-10**: 10cmd/min
- **フォロー IF-016-11**: `deployment/rollback.md`

#### シナリオ IF-017: Transport Segment API MTU契約
- **対象 IF-017-01**: Obf→Transport
- **バージョン IF-017-02**: 1.2
- **入力 IF-017-03**: payload=1260B
- **期待 IF-017-04**: Segment送信成功
- **契約 IF-017-05**: MTU=1280B, overhead≤20B
- **エラー制御 IF-017-06**: 超過時ERR-TSP-431
- **互換性 IF-017-07**: Transport v1
- **監査 IF-017-08**: Telemetry
- **検証 IF-017-09**: パフォーマンス
- **レート IF-017-10**: 5kseg/s
- **フォロー IF-017-11**: 指標更新

#### シナリオ IF-018: Transport Segment 再送通知
- **対象 IF-018-01**: Transport→Obf
- **バージョン IF-018-02**: 1.2
- **入力 IF-018-03**: segment_loss=true
- **期待 IF-018-04**: `SEGMENT_NACK`
- **契約 IF-018-05**: `retry_after_ms`提供
- **エラー制御 IF-018-06**: NACK未送時警告
- **互換性 IF-018-07**: Obf v1
- **監査 IF-018-08**: ロスログ
- **検証 IF-018-09**: Lossシナリオ
- **レート IF-018-10**: 100nack/min
- **フォロー IF-018-11**: `performance/scalability.md`

#### シナリオ IF-019: Multipath Policy Interface
- **対象 IF-019-01**: Control→Stream Multipath設定
- **バージョン IF-019-02**: 1.0
- **入力 IF-019-03**: `enabled=true, path_limit=3`
- **期待 IF-019-04**: Streamが3経路まで許容
- **契約 IF-019-05**: path_limit<=5
- **エラー制御 IF-019-06**: 超過時422
- **互換性 IF-019-07**: Multipathプラグイン
- **監査 IF-019-08**: Policyログ
- **検証 IF-019-09**: Multipathテスト
- **レート IF-019-10**: 15cmd/min
- **フォロー IF-019-11**: `formal/nyx_multipath_plugin.cfg`

#### シナリオ IF-020: Multipath Policy Disabled
- **対象 IF-020-01**: Control→Stream
- **バージョン IF-020-02**: 1.0
- **入力 IF-020-03**: `enabled=false`
- **期待 IF-020-04**: Streamが単一路線へ戻す
- **契約 IF-020-05**: 過去経路をGraceful終了
- **エラー制御 IF-020-06**: 無
- **互換性 IF-020-07**: Stream v2
- **監査 IF-020-08**: Policyログ
- **検証 IF-020-09**: 通信テスト
- **レート IF-020-10**: 15cmd/min
- **フォロー IF-020-11**: Runbook

#### シナリオ IF-021: Admin API フィルタ複合条件
- **対象 IF-021-01**: Audit Query
- **バージョン IF-021-02**: 1.1
- **入力 IF-021-03**: actor+severity複合
- **期待 IF-021-04**: 交差条件適用
- **契約 IF-021-05**: 空集合時204
- **エラー制御 IF-021-06**: 無
- **互換性 IF-021-07**: 管理コンソール
- **監査 IF-021-08**: Queryログ
- **検証 IF-021-09**: APIテスト
- **レート IF-021-10**: 5req/min
- **フォロー IF-021-11**: ドキュメント追記

#### シナリオ IF-022: Handshake API ロケールサポート拡張
- **対象 IF-022-01**: `POST /nyx/v1/session`
- **バージョン IF-022-02**: 1.3
- **入力 IF-022-03**: locale=ar
- **期待 IF-022-04**: 右→左対応フラグ
- **契約 IF-022-05**: locale ISO準拠
- **エラー制御 IF-022-06**: 未サポート時406
- **互換性 IF-022-07**: UI
- **監査 IF-022-08**: ロケールログ
- **検証 IF-022-09**: i18n
- **レート IF-022-10**: 通常
- **フォロー IF-022-11**: UIテンプレ更新

#### シナリオ IF-023: Telemetry API 圧縮サポート
- **対象 IF-023-01**: `PUT /telemetry/v1/metrics`
- **バージョン IF-023-02**: 1.4
- **入力 IF-023-03**: Content-Encoding=gzip
- **期待 IF-023-04**: 正常解析
- **契約 IF-023-05**: 圧縮サイズ<=1MB
- **エラー制御 IF-023-06**: 解凍失敗時415
- **互換性 IF-023-07**: Telemetry v1.4
- **監査 IF-023-08**: 圧縮利用率
- **検証 IF-023-09**: パフォーマンス
- **レート IF-023-10**: 80req/min
- **フォロー IF-023-11**: ObservabilityDocs

#### シナリオ IF-024: Telemetry API ラベル衝突
- **対象 IF-024-01**: `PUT /telemetry/v1/metrics`
- **バージョン IF-024-02**: 1.4
- **入力 IF-024-03**: 同一ラベル重複
- **期待 IF-024-04**: 422 + エラー詳細
- **契約 IF-024-05**: 重複禁止
- **エラー制御 IF-024-06**: 重複キー列挙
- **互換性 IF-024-07**: すべて
- **監査 IF-024-08**: Validationログ
- **検証 IF-024-09**: APIテスト
- **レート IF-024-10**: 制限無し
- **フォロー IF-024-11**: SDK修正

#### シナリオ IF-025: Client Handshake Capabilityネゴシエーション
- **対象 IF-025-01**: `POST /nyx/v1/session`
- **バージョン IF-025-02**: 1.4
- **入力 IF-025-03**: capability=`MULTIPATH,VISION`
- **期待 IF-025-04**: 非対応`VISION`を`unsupported_capabilities`へ返却
- **契約 IF-025-05**: `unsupported_capabilities`配列
- **エラー制御 IF-025-06**: 200
- **互換性 IF-025-07**: SDK多機能
- **監査 IF-025-08**: Capabilityログ
- **検証 IF-025-09**: 契約テスト
- **レート IF-025-10**: 30req/min
- **フォロー IF-025-11**: `capability_negotiation_traceability.md`

#### シナリオ IF-026: Handshake API レート制限超過
- **対象 IF-026-01**: `POST /nyx/v1/session`
- **バージョン IF-026-02**: 1.4
- **入力 IF-026-03**: 60req/min
- **期待 IF-026-04**: 429 + Retry-After
- **契約 IF-026-05**: リミット=30
- **エラー制御 IF-026-06**: 冷却要求
- **互換性 IF-026-07**: SDK
- **監査 IF-026-08**: Rateログ
- **検証 IF-026-09**: 負荷テスト
- **レート IF-026-10**: 超過
- **フォロー IF-026-11**: Runbook

#### シナリオ IF-027: Stream API バージョンネゴ失敗
- **対象 IF-027-01**: Stream handshake
- **バージョン IF-027-02**: Client=2.1, Server=1.9
- **入力 IF-027-03**: サポート差異
- **期待 IF-027-04**: 交渉で1.9
- **契約 IF-027-05**: `acceptable_versions`
- **エラー制御 IF-027-06**: 互換無なら426
- **互換性 IF-027-07**: SDK
- **監査 IF-027-08**: 交渉ログ
- **検証 IF-027-09**: バージョンテスト
- **レート IF-027-10**: 通常
- **フォロー IF-027-11**: `compatibility.yaml`

#### シナリオ IF-028: ROUTE_PLAN タイムアウトエラー
- **対象 IF-028-01**: Stream→Mix
- **バージョン IF-028-02**: 2.0
- **入力 IF-028-03**: ノード不足
- **期待 IF-028-04**: `ERR-ROUTE-503`
- **契約 IF-028-05**: RetryAfter=5
- **エラー制御 IF-028-06**: 3回まで
- **互換性 IF-028-07**: Stream v2
- **監査 IF-028-08**: Route失敗ログ
- **検証 IF-028-09**: 失敗テスト
- **レート IF-028-10**: 制限内
- **フォロー IF-028-11**: Capacity計画

#### シナリオ IF-029: POLICY_UPDATE シグネチャ検証
- **対象 IF-029-01**: Control Broadcast
- **バージョン IF-029-02**: 1.6
- **入力 IF-029-03**: 不正署名
- **期待 IF-029-04**: 全ノード拒否
- **契約 IF-029-05**: mTLS+JWT
- **エラー制御 IF-029-06**: Err401
- **互換性 IF-029-07**: 全層
- **監査 IF-029-08**: Securityログ
- **検証 IF-029-09**: セキュリティテスト
- **レート IF-029-10**: 10event/min
- **フォロー IF-029-11**: `security/auth.md`

#### シナリオ IF-030: Telemetry API 受信順序保証
- **対象 IF-030-01**: `PUT /telemetry/v1/metrics`
- **バージョン IF-030-02**: 1.5
- **入力 IF-030-03**: out-of-order timestamp
- **期待 IF-030-04**: サーバで整列
- **契約 IF-030-05**: `accept_out_of_order=true`
- **エラー制御 IF-030-06**: 0
- **互換性 IF-030-07**: Observability v2
- **監査 IF-030-08**: reorderログ
- **検証 IF-030-09**: Integration
- **レート IF-030-10**: 120req/min
- **フォロー IF-030-11**: ダッシュボード調整

#### シナリオ IF-031: Admin API Audit重複排除
- **対象 IF-031-01**: `GET /governance/v1/audit-events`
- **バージョン IF-031-02**: 1.2
- **入力 IF-031-03**: actor重複
- **期待 IF-031-04**: 重複排除
- **契約 IF-031-05**: `deduplicate=true`
- **エラー制御 IF-031-06**: 0
- **互換性 IF-031-07**: GovPortal
- **監査 IF-031-08**: Queryログ
- **検証 IF-031-09**: APIテスト
- **レート IF-031-10**: 5req/min
- **フォロー IF-031-11**: Spec追記

#### シナリオ IF-032: Stream Frame QoSタグ検証
- **対象 IF-032-01**: Stream→Mix
- **バージョン IF-032-02**: 2.2
- **入力 IF-032-03**: QoSタグ不正
- **期待 IF-032-04**: ERR-FRAME-428
- **契約 IF-032-05**: QoS値限定
- **エラー制御 IF-032-06**: 422
- **互換性 IF-032-07**: Stream v2.2
- **監査 IF-032-08**: QoSログ
- **検証 IF-032-09**: 単体
- **レート IF-032-10**: 通常
- **フォロー IF-032-11**: QoS仕様更新

#### シナリオ IF-033: Stream Frame Prioritization成功
- **対象 IF-033-01**: Stream→Mix
- **バージョン IF-033-02**: 2.2
- **入力 IF-033-03**: priority=HIGH
- **期待 IF-033-04**: Mixが優先処理
- **契約 IF-033-05**: latency90p%≤120ms
- **エラー制御 IF-033-06**: 遅延悪化時Throttle
- **互換性 IF-033-07**: Mix v2.1+
- **監査 IF-033-08**: QoSイベント
- **検証 IF-033-09**: パフォーマンス
- **レート IF-033-10**: 800frames/s
- **フォロー IF-033-11**: `performance/qos.md`

#### シナリオ IF-034: Stream Frame 優先度降格
- **対象 IF-034-01**: Stream→Mix
- **バージョン IF-034-02**: 2.2
- **入力 IF-034-03**: priority=LOW, latencySLO違反
- **期待 IF-034-04**: 自動降格拒否
- **契約 IF-034-05**: priority保持
- **エラー制御 IF-034-06**: Warning
- **互換性 IF-034-07**: Mix v2系
- **監査 IF-034-08**: QoSログ
- **検証 IF-034-09**: SLAテスト
- **レート IF-034-10**: 通常
- **フォロー IF-034-11**: QoS Runbook

#### シナリオ IF-035: Telemetry API メトリクス欠損
- **対象 IF-035-01**: `PUT /telemetry/v1/metrics`
- **バージョン IF-035-02**: 1.5
- **入力 IF-035-03**: metric_bundle空
- **期待 IF-035-04**: 204 No Content
- **契約 IF-035-05**: 空許容
- **エラー制御 IF-035-06**: 無
- **互換性 IF-035-07**: SDK
- **監査 IF-035-08**: Telemetry
- **検証 IF-035-09**: Nullケース
- **レート IF-035-10**: 制限無
- **フォロー IF-035-11**: SDKガイド

#### シナリオ IF-036: Telemetry API メトリクス過大
- **対象 IF-036-01**: `PUT /telemetry/v1/metrics`
- **バージョン IF-036-02**: 1.5
- **入力 IF-036-03**: metric_bundle=1500
- **期待 IF-036-04**: 413 Payload Too Large
- **契約 IF-036-05**: 1000件上限
- **エラー制御 IF-036-06**: `split_hint`
- **互換性 IF-036-07**: Observability
- **監査 IF-036-08**: Rateログ
- **検証 IF-036-09**: 境界テスト
- **レート IF-036-10**: 制限内
- **フォロー IF-036-11**: 運用Runbook

#### シナリオ IF-037: Audit API 監査証跡整合
- **対象 IF-037-01**: `GET /governance/v1/audit-events`
- **バージョン IF-037-02**: 1.3
- **入力 IF-037-03**: `action=SESSION_CREATE`
- **期待 IF-037-04**: AuditとTelemetry整合
- **契約 IF-037-05**: event_id=UUIDv7
- **エラー制御 IF-037-06**: 整合性違反時アラート
- **互換性 IF-037-07**: GovPortal
- **監査 IF-037-08**: Complianceログ
- **検証 IF-037-09**: 監査テスト
- **レート IF-037-10**: 5req/min
- **フォロー IF-037-11**: コンプライアンス報告

#### シナリオ IF-038: Audit API GDPRマスキング
- **対象 IF-038-01**: `GET /governance/v1/audit-events`
- **バージョン IF-038-02**: 1.3
- **入力 IF-038-03**: EUリージョン
- **期待 IF-038-04**: PIIマスキング
- **契約 IF-038-05**: `gdpr_masking=true`
- **エラー制御 IF-038-06**: 無
- **互換性 IF-038-07**: EU準拠
- **監査 IF-038-08**: GDPRログ
- **検証 IF-038-09**: コンプライアンステスト
- **レート IF-038-10**: 規定
- **フォロー IF-038-11**: 合規文書更新

#### シナリオ IF-039: Handshake API 切断検知
- **対象 IF-039-01**: `POST /nyx/v1/session`
- **バージョン IF-039-02**: 1.5
- **入力 IF-039-03**: `disconnect_notify=true`
- **期待 IF-039-04**: Webhook登録
- **契約 IF-039-05**: Webhook応答≤5s
- **エラー制御 IF-039-06**: 登録失敗時再試行3回
- **互換性 IF-039-07**: SDK新機能
- **監査 IF-039-08**: Webhookログ
- **検証 IF-039-09**: Integration
- **レート IF-039-10**: 10req/min
- **フォロー IF-039-11**: Webhookガイド

#### シナリオ IF-040: Handshake API トークン再発行
- **対象 IF-040-01**: `POST /nyx/v1/session`
- **バージョン IF-040-02**: 1.5
- **入力 IF-040-03**: `renew_token=true`
- **期待 IF-040-04**: 新旧トークン併存300秒
- **契約 IF-040-05**: TTL重複許容
- **エラー制御 IF-040-06**: 429防止
- **互換性 IF-040-07**: SDK, CLI
- **監査 IF-040-08**: Renewalイベント
- **検証 IF-040-09**: 契約テスト
- **レート IF-040-10**: 20req/min
- **フォロー IF-040-11**: トークンRunbook

#### シナリオ IF-041: Stream API トポロジ変更通知
- **対象 IF-041-01**: Stream→Client通知
- **バージョン IF-041-02**: 2.3
- **入力 IF-041-03**: route_change=true
- **期待 IF-041-04**: `ROUTE_UPDATE`
- **契約 IF-041-05**: 5秒以内通知
- **エラー制御 IF-041-06**: 未送時アラート
- **互換性 IF-041-07**: SDK v2.3
- **監査 IF-041-08**: Telemetry
- **検証 IF-041-09**: Integration
- **レート IF-041-10**: 100event/min
- **フォロー IF-041-11**: `architecture/overview.md`

#### シナリオ IF-042: Stream API リンク断絶処理
- **対象 IF-042-01**: Stream→Clientエラー
- **バージョン IF-042-02**: 2.3
- **入力 IF-042-03**: link_loss=true
- **期待 IF-042-04**: `LINK_LOSS`通知 + 再接続情報
- **契約 IF-042-05**: 再接続ガイドTTL=60s
- **エラー制御 IF-042-06**: 3回再試行
- **互換性 IF-042-07**: SDK v2.3
- **監査 IF-042-08**: Lossログ
- **検証 IF-042-09**: フェイルオーバーテスト
- **レート IF-042-10**: 80event/min
- **フォロー IF-042-11**: フェイルオーバーRunbook

#### シナリオ IF-043: Obfuscation API カバレッジ再調整
- **対象 IF-043-01**: Control→Obfuscation
- **バージョン IF-043-02**: 3.1
- **入力 IF-043-03**: cover_ratio=0.35
- **期待 IF-043-04**: ACK + 適用結果
- **契約 IF-043-05**: ratio範囲0.1-0.6
- **エラー制御 IF-043-06**: 範囲外422
- **互換性 IF-043-07**: Obf v2+
- **監査 IF-043-08**: Policyログ
- **検証 IF-043-09**: 負荷テスト
- **レート IF-043-10**: 15cmd/min
- **フォロー IF-043-11**: `adaptive_cover_traffic_spec.md`

#### シナリオ IF-044: Obfuscation API 遅延目標調整
- **対象 IF-044-01**: Control→Obfuscation
- **バージョン IF-044-02**: 3.1
- **入力 IF-044-03**: latency_budget_ms=250
- **期待 IF-044-04**: 新予算適用
- **契約 IF-044-05**: budget範囲100-400
- **エラー制御 IF-044-06**: 違反時エラー422
- **互換性 IF-044-07**: Obf v2+
- **監査 IF-044-08**: Telemetry
- **検証 IF-044-09**: 遅延テスト
- **レート IF-044-10**: 15cmd/min
- **フォロー IF-044-11**: SLO文書

#### シナリオ IF-045: Transport API パス切替
- **対象 IF-045-01**: Transport→Stream通知
- **バージョン IF-045-02**: 1.3
- **入力 IF-045-03**: path_switch=true
- **期待 IF-045-04**: `PATH_SWITCH` + 新経路
- **契約 IF-045-05**: Switch完了≤2s
- **エラー制御 IF-045-06**: 失敗時再試行
- **互換性 IF-045-07**: Stream v2
- **監査 IF-045-08**: Telemetry
- **検証 IF-045-09**: Multipathテスト
- **レート IF-045-10**: 200event/min
- **フォロー IF-045-11**: MultipathRunbook

#### シナリオ IF-046: Transport API 帯域不足通知
- **対象 IF-046-01**: Transport→Stream
- **バージョン IF-046-02**: 1.3
- **入力 IF-046-03**: bandwidth_drop=40%
- **期待 IF-046-04**: `BANDWIDTH_ALERT`
- **契約 IF-046-05**: 連続発生で警告レベル昇格
- **エラー制御 IF-046-06**: スロットリング指示
- **互換性 IF-046-07**: Stream v2
- **監査 IF-046-08**: Telemetry
- **検証 IF-046-09**: 負荷テスト
- **レート IF-046-10**: 150event/min
- **フォロー IF-046-11**: Capacity計画

#### シナリオ IF-047: Governance API 証跡エクスポート
- **対象 IF-047-01**: `POST /governance/v1/export`
- **バージョン IF-047-02**: 1.0
- **入力 IF-047-03**: export_format=CSV
- **期待 IF-047-04**: 202 Accepted + download_url
- **契約 IF-047-05**: URL有効期限24h
- **エラー制御 IF-047-06**: 再発行制限
- **互換性 IF-047-07**: GovPortal
- **監査 IF-047-08**: Exportログ
- **検証 IF-047-09**: エクスポートテスト
- **レート IF-047-10**: 2req/min
- **フォロー IF-047-11**: コンプライアンス通知

#### シナリオ IF-048: Governance API 監査署名
- **対象 IF-048-01**: `POST /governance/v1/export`
- **バージョン IF-048-02**: 1.0
- **入力 IF-048-03**: signature_format=CMS
- **期待 IF-048-04**: 署名ファイル提供
- **契約 IF-048-05**: CMSベース
- **エラー制御 IF-048-06**: 署名失敗時再試行1回
- **互換性 IF-048-07**: 監査機関
- **監査 IF-048-08**: 署名ログ
- **検証 IF-048-09**: セキュリティレビュー
- **レート IF-048-10**: 2req/min
- **フォロー IF-048-11**: 監査ポリシー追記

#### シナリオ IF-049: CLI ハンドシェイク契約
- **対象 IF-049-01**: `nyx-cli session create`
- **バージョン IF-049-02**: CLI>=1.8
- **入力 IF-049-03**: `--pq --locale en`
- **期待 IF-049-04**: API contract準拠
- **契約 IF-049-05**: CLI出力にTTL表示
- **エラー制御 IF-049-06**: API 429時リトライ
- **互換性 IF-049-07**: API v1.4
- **監査 IF-049-08**: CLI Telemetry
- **検証 IF-049-09**: CLI契約テスト
- **レート IF-049-10**: CLI利用頻度内
- **フォロー IF-049-11**: CLIドキュメント

#### シナリオ IF-050: SDK ハンドシェイク契約
- **対象 IF-050-01**: `nyx-sdk` Handshakeクラス
- **バージョン IF-050-02**: SDK>=2.0
- **入力 IF-050-03**: `capabilities=['MULTIPATH']`
- **期待 IF-050-04**: APIと契約整合
- **契約 IF-050-05**: SDKラッパーで429再試行1回
- **エラー制御 IF-050-06**: エラーハンドラ発火
- **互換性 IF-050-07**: API v1.4
- **監査 IF-050-08**: SDK Telemetry
- **検証 IF-050-09**: SDK契約テスト
- **レート IF-050-10**: SDK利用頻度内
- **フォロー IF-050-11**: SDKリリースノート

## 関連ドキュメント
- [architecture/dataflow.md](./dataflow.md)
- [security/auth.md](../security/auth.md)
- [testing/integration-tests.md](../testing/integration-tests.md)
- [templates/module-template.md](../templates/module-template.md)

> **宣言**: 本章はあくまで言語非依存のI/F仕様であり、実装コードやC/C++依存ライブラリを含まない。