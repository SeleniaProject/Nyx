# docs/security/auth.md

> **遵守バッジ** : :no_entry: 実装コード非出力 / :no_entry_sign: C/C++依存禁止

## 目次
- [目的](#目的)
- [認証フレームワーク](#認証フレームワーク)
- [認可モデル](#認可モデル)
- [セッション管理](#セッション管理)
- [権限制御](#権限制御)
- [監査と可観測性](#監査と可観測性)
- [ポリシーライフサイクル](#ポリシーライフサイクル)
- [テストと検証](#テストと検証)
- [拡張補遺: 認証・認可保障シナリオ集](#拡張補遺-認証認可保障シナリオ集)
-   - [シナリオ AUTH-001: 標準OIDCフロー検証](#シナリオ-auth-001-標準oidcフロー検証)
-   - [シナリオ AUTH-002: PKCEパラメータ欠落](#シナリオ-auth-002-pkceパラメータ欠落)
-   - [シナリオ AUTH-003: mTLS証明書ローテーション](#シナリオ-auth-003-mtls証明書ローテーション)
-   - [シナリオ AUTH-004: 管理者MFA強制](#シナリオ-auth-004-管理者mfa強制)
-   - [シナリオ AUTH-005: デバイス証明書失効](#シナリオ-auth-005-デバイス証明書失効)
-   - [シナリオ AUTH-006: RBAC役割昇格ワークフロー](#シナリオ-auth-006-rbac役割昇格ワークフロー)
-   - [シナリオ AUTH-007: ABACタイムウィンドウ](#シナリオ-auth-007-abacタイムウィンドウ)
-   - [シナリオ AUTH-008: OPAポリシー冪等性](#シナリオ-auth-008-opaポリシー冪等性)
-   - [シナリオ AUTH-009: セッションタイムアウト境界](#シナリオ-auth-009-セッションタイムアウト境界)
-   - [シナリオ AUTH-010: リフレッシュトークン再発行](#シナリオ-auth-010-リフレッシュトークン再発行)
-   - [シナリオ AUTH-011: セッションハイジャック検出](#シナリオ-auth-011-セッションハイジャック検出)
-   - [シナリオ AUTH-012: Auditイベント完全性](#シナリオ-auth-012-auditイベント完全性)
-   - [シナリオ AUTH-013: フェデレーションIdP障害](#シナリオ-auth-013-フェデレーションidp障害)
-   - [シナリオ AUTH-014: サービスアカウントローテーション](#シナリオ-auth-014-サービスアカウントローテーション)
-   - [シナリオ AUTH-015: mTLS失敗時フォールバック禁止](#シナリオ-auth-015-mtls失敗時フォールバック禁止)
-   - [シナリオ AUTH-016: セッションブラックリスト試験](#シナリオ-auth-016-セッションブラックリスト試験)
-   - [シナリオ AUTH-017: OAuth Scopes整合性](#シナリオ-auth-017-oauth-scopes整合性)
-   - [シナリオ AUTH-018: 管理者緊急アクセス手順](#シナリオ-auth-018-管理者緊急アクセス手順)
-   - [シナリオ AUTH-019: 自動ポリシー撤回](#シナリオ-auth-019-自動ポリシー撤回)
-   - [シナリオ AUTH-020: 監査ログ配送遅延](#シナリオ-auth-020-監査ログ配送遅延)
-   - [シナリオ AUTH-021: 多地域認証遅延](#シナリオ-auth-021-多地域認証遅延)
-   - [シナリオ AUTH-022: ハードウェアキー更新](#シナリオ-auth-022-ハードウェアキー更新)
-   - [シナリオ AUTH-023: CLI認証フロー検証](#シナリオ-auth-023-cli認証フロー検証)
-   - [シナリオ AUTH-024: SDKトークンハンドリング](#シナリオ-auth-024-sdkトークンハンドリング)
-   - [シナリオ AUTH-025: 多要素疲労攻撃対策](#シナリオ-auth-025-多要素疲労攻撃対策)
-   - [シナリオ AUTH-026: セッション再認証閾値](#シナリオ-auth-026-セッション再認証閾値)
-   - [シナリオ AUTH-027: RBACロール削除影響](#シナリオ-auth-027-rbacロール削除影響)
-   - [シナリオ AUTH-028: ABAC属性キャッシング](#シナリオ-auth-028-abac属性キャッシング)
-   - [シナリオ AUTH-029: OPAルール競合検知](#シナリオ-auth-029-opaルール競合検知)
-   - [シナリオ AUTH-030: Audit不可視イベント監査](#シナリオ-auth-030-audit不可視イベント監査)
-   - [シナリオ AUTH-031: IdP証明書期限切れ警告](#シナリオ-auth-031-idp証明書期限切れ警告)
-   - [シナリオ AUTH-032: セッションキー同期失敗](#シナリオ-auth-032-セッションキー同期失敗)
-   - [シナリオ AUTH-033: 高感度リソースアクセス](#シナリオ-auth-033-高感度リソースアクセス)
-   - [シナリオ AUTH-034: APIトークン権限縮小](#シナリオ-auth-034-apiトークン権限縮小)
-   - [シナリオ AUTH-035: セッション継続監査](#シナリオ-auth-035-セッション継続監査)
-   - [シナリオ AUTH-036: フェデレーションメタデータ更新](#シナリオ-auth-036-フェデレーションメタデータ更新)
-   - [シナリオ AUTH-037: アクセストークン暗号強度評価](#シナリオ-auth-037-アクセストークン暗号強度評価)
-   - [シナリオ AUTH-038: Policyロールバック検証](#シナリオ-auth-038-policyロールバック検証)
-   - [シナリオ AUTH-039: スタッフ離任時アクセス遮断](#シナリオ-auth-039-スタッフ離任時アクセス遮断)
-   - [シナリオ AUTH-040: 分散監査ログ整合性](#シナリオ-auth-040-分散監査ログ整合性)
-   - [シナリオ AUTH-041: SSOダウン時代替経路](#シナリオ-auth-041-ssoダウン時代替経路)
-   - [シナリオ AUTH-042: セッションレート制限逸脱](#シナリオ-auth-042-セッションレート制限逸脱)
-   - [シナリオ AUTH-043: デバイス紛失対応](#シナリオ-auth-043-デバイス紛失対応)
-   - [シナリオ AUTH-044: APIキー最小権限確認](#シナリオ-auth-044-apiキー最小権限確認)
-   - [シナリオ AUTH-045: RBAC監査証跡整合](#シナリオ-auth-045-rbac監査証跡整合)
-   - [シナリオ AUTH-046: ABAC属性ソース障害](#シナリオ-auth-046-abac属性ソース障害)
-   - [シナリオ AUTH-047: OPAデプロイロールアウト](#シナリオ-auth-047-opaデプロイロールアウト)
-   - [シナリオ AUTH-048: セッションメタデータ完全性](#シナリオ-auth-048-セッションメタデータ完全性)
-   - [シナリオ AUTH-049: 監査アラートノイズ削減](#シナリオ-auth-049-監査アラートノイズ削減)
-   - [シナリオ AUTH-050: 緊急時アクセス放棄確認](#シナリオ-auth-050-緊急時アクセス放棄確認)
-   - [シナリオ運用サマリ](#シナリオ運用サマリ)
-   - [保証メトリクスダッシュボード](#保証メトリクスダッシュボード)
-   - [継続的改善バックログ](#継続的改善バックログ)
- [関連ドキュメント](#関連ドキュメント)

## 目的
Nyxプラットフォームにおける認証・認可・監査の設計を定義し、ゼロトラスト原則を実現する。

## 認証フレームワーク
- **OIDC/OAuth2**: Keycloak等のIdPを利用し、Authorization Code Flow + PKCE を標準。
- **mTLS**: ノード間通信は相互TLSで相互認証。
- **デバイス認証**: モバイル端末はデバイス証明書 + ハードウェアバックdKeyストア。
- **多要素認証**: 管理者/監査人はMFA必須。TOTPまたはFIDO2。

## 認可モデル
| モデル | 用途 | 実装方針 |
|--------|------|----------|
| RBAC | UI/管理操作 | 役割: Viewer, Operator, Admin, Auditor |
| ABAC | APIレベル細分化 | 属性: 組織、感度、時間帯 |
| Policy-as-Code | ガバナンス | OPA/Rego等のマネージド実装を利用（C依存なし） |

## セッション管理
- セッションは0-RTTで確立。`session_token`はJWT (短期) + リフレッシュトークン (長期) 組合せ。
- キー回転: 10分または1GBデータ送信で更新 ([security/encryption.md](./encryption.md))。
- セッション失効は集中管理。ブラックリストではなく短寿命トークンによる失効モデル。

## 権限制御
| 操作 | RBAC要件 | ABAC条件 | トレーサビリティ |
|------|----------|----------|------------------|
| セッション設定編集 | Operator以上 | 組織=一致、時間=営業 | REQ-FUN-040 |
| 監査ログ閲覧 | Auditor | 感度`<=Confidential` | REQ-FUN-041 |
| ポリシー変更 | Admin | 緊急フラグ=否 | ADR-XXXX |

## 監査と可観測性
- すべての認証試行は`AUTH_EVENT`としてAudit Busへ送信。
- 成功/失敗イベントを区別し、不審な行動 (連続失敗) を検知。
- レポートは[deployment/ci-cd.md](../deployment/ci-cd.md)に連携し、自動監視。

## ポリシーライフサイクル
1. 要件定義: [requirements.md](../requirements.md)に基づく。
2. モデル化: Policy-as-Codeで記述。
3. デプロイ: Canary適用 → 本番展開。
4. レビュー: [notes/decision-log.md](../notes/decision-log.md)に記録。
5. 監査: 四半期ごとに監査人レビュー。

## テストと検証
- 契約テスト: [testing/integration-tests.md](../testing/integration-tests.md)でRBAC/ABACシナリオを網羅。
- セキュリティテスト: ペネトレーションテスト計画は[security/vulnerability.md](./vulnerability.md)。
- モデル検証: Policy-as-Codeは静的解析とシミュレーションを実施。

## 拡張補遺: 認証・認可保障シナリオ集
本補遺はNyx認証・認可領域における保証アクティビティを体系化するためのシナリオカタログである。各シナリオは実行タイミング、トレーサビリティ、運用保守責任を明示し、[docs/performance/scalability.md](../performance/scalability.md)や[docs/architecture/dependencies.md](../architecture/dependencies.md)に整合する。以下のケースは四半期計画レビューの入力、および[security/vulnerability.md](./vulnerability.md)で定義したリスクマトリクスへの反映が求められる。

### シナリオ AUTH-001: 標準OIDCフロー検証
- **対象領域**: Authorization Code Flow + PKCE
- **脅威仮説**: 認可コード盗難によるセッション乗っ取り
- **前提**: 正常系IdP、推奨TLS設定、[architecture/interfaces.md](../architecture/interfaces.md)準拠
- **検証手順**: CLI + Webクライアントから同一ユーザーでフロー実行し、PKCE検証とnonce確認をエンドツーエンドで確認
- **期待アウトカム**: IdPとNyx API間のコード交換が1回のみで終了し、認可コード再利用が拒否される
- **メトリクス/SLI**: `auth.success_rate` ≥ 99.95%、`auth.pkce_validation_failure` = 0
- **失敗時対応**: [runbooks/auth-flow-reset.md](../../runbooks/auth-flow-reset.md)でコードキャッシュ無効化
- **参照Runbook**: `RB-AUTH-001`
- **トレーサビリティ**: REQ-AUTH-001, ADR-AUTH-005

### シナリオ AUTH-002: PKCEパラメータ欠落
- **対象領域**: PKCE `code_verifier`サニティチェック
- **脅威仮説**: クライアント実装不備による弱い認証コード交換
- **前提**: バージョン互換性検証済みSDK ([nyx-sdk/README.md](../../nyx-sdk/README.md))
- **検証手順**: 自動テストで`code_verifier`欠落リクエストを送信し、IdPおよびNyx APIの拒否レスポンスを確認
- **期待アウトカム**: HTTP 400 + エラーログに`PKCE_REQUIRED`が出力される
- **メトリクス/SLI**: `auth.invalid_pkce_request`アラートが1分以内に発火
- **失敗時対応**: SDKバージョンピン留めを`nyx.toml`で更新し、回収リリースを適用
- **参照Runbook**: `RB-AUTH-012`
- **トレーサビリティ**: REQ-AUTH-011, RSK-AUTH-003

### シナリオ AUTH-003: mTLS証明書ローテーション
- **対象領域**: サービス間mTLSハンドシェイク
- **脅威仮説**: 期限切れ証明書による通信遮断とセキュリティギャップ
- **前提**: [security/encryption.md](./encryption.md)で定義されたローテーションスケジュール遵守
- **検証手順**: 新旧証明書を`nyx-daemon`に順次導入し、ハンドシェイク成功と失効後の拒否を確認
- **期待アウトカム**: 有効期間切替時にゼロダウンタイム、失効証明書は即時拒否
- **メトリクス/SLI**: `mtls.handshake_success_rate` ≥ 99.9%、`cert.expiry_lead_time_hours` ≥ 168
- **失敗時対応**: [runbooks/certificate-recovery.md](../../runbooks/certificate-recovery.md)で失効対応
- **参照Runbook**: `RB-CRT-002`
- **トレーサビリティ**: ADR-SEC-004, REQ-CRYPTO-007

### シナリオ AUTH-004: 管理者MFA強制
- **対象領域**: 管理者向け多要素認証
- **脅威仮説**: 単要素認証での権限奪取
- **前提**: 管理者アカウントにFIDO2が登録済み
- **検証手順**: 管理者UIログインでMFAスキップ要求を送信し拒否ログとポリシー適用をチェック
- **期待アウトカム**: スキップ試行はHTTP 403で拒否され、監査ログに`MFA_REQUIRED`が記録
- **メトリクス/SLI**: `mfa.enforcement_rate` = 100%、`mfa.skip_attempts`の検知遅延 < 30秒
- **失敗時対応**: [runbooks/mfa-enforcement.md](../../runbooks/mfa-enforcement.md)によるポリシー再同期
- **参照Runbook**: `RB-MFA-001`
- **トレーサビリティ**: REQ-AUTH-020, ADR-SEC-010

### シナリオ AUTH-005: デバイス証明書失効
- **対象領域**: モバイル端末証明書とデバイス姿勢
- **脅威仮説**: 失効済み証明書による不正アクセス
- **前提**: [nyx-mobile-ffi](../../nyx-mobile-ffi/README.md)が最新版、CRL/OCSPが稼働
- **検証手順**: 失効済み証明書で接続し、CRLチェックとOCSPレスポンスを観測
- **期待アウトカム**: 失効検知でアクセス拒否、監査ログに`DEVICE_CERT_REVOKED`
- **メトリクス/SLI**: `device.revocation_detection_latency` ≤ 15秒
- **失敗時対応**: [runbooks/device-revocation.md](../../runbooks/device-revocation.md)で端末隔離
- **参照Runbook**: `RB-DEV-004`
- **トレーサビリティ**: RSK-MBL-002, REQ-AUTH-034

### シナリオ AUTH-006: RBAC役割昇格ワークフロー
- **対象領域**: RBACロール昇格申請
- **脅威仮説**: 不適切な昇格手続きによる権限濫用
- **前提**: [architecture/dependencies.md](../architecture/dependencies.md)で定義した承認フローが有効
- **検証手順**: 昇格申請→承認→監査ログを通し、2段階承認と失効タイマーを確認
- **期待アウトカム**: 2名承認後に限定期間ロール付与、期限到達で自動剥奪
- **メトリクス/SLI**: `rbac.elevation_dual_approval_rate` = 100%
- **失敗時対応**: [runbooks/rbac-revoke.md](../../runbooks/rbac-revoke.md)で即時ロール剥奪
- **参照Runbook**: `RB-RBAC-007`
- **トレーサビリティ**: REQ-RBAC-010, AUD-SEC-003

### シナリオ AUTH-007: ABACタイムウィンドウ
- **対象領域**: ABAC時間属性評価
- **脅威仮説**: 時間属性不整合によるアクセス逸脱
- **前提**: 時刻同期が[architecture/tech-stack.md](../architecture/tech-stack.md)で規定するNTPクラスタに整合
- **検証手順**: 営業時間外アクセスをシミュレートしポリシー拒否と監査記録を確認
- **期待アウトカム**: ABACエンジンが`time_window_violation`を返却
- **メトリクス/SLI**: `abac.time_drift_seconds` ≤ 5
- **失敗時対応**: [runbooks/abac-sync.md](../../runbooks/abac-sync.md)で時間属性再同期
- **参照Runbook**: `RB-ABAC-002`
- **トレーサビリティ**: REQ-ABAC-006, ADR-ABAC-001

### シナリオ AUTH-008: OPAポリシー冪等性
- **対象領域**: Policy-as-Codeデプロイパイプライン
- **脅威仮説**: 重複デプロイでポリシー逸脱
- **前提**: OPAバンドル署名が[security/policy-distribution.md](./policy-distribution.md)で有効
- **検証手順**: 同一バンドルを連続デプロイしバージョンIDとハッシュ一致を検証
- **期待アウトカム**: 冪等適用でポリシーチェックサムが変化せず、重複適用ログのみ
- **メトリクス/SLI**: `opa.bundle_apply_duration` ≤ 10秒
- **失敗時対応**: [runbooks/policy-rollback.md](../../runbooks/policy-rollback.md)で安全ロールバック
- **参照Runbook**: `RB-OPA-003`
- **トレーサビリティ**: ADR-OPA-002, REQ-POL-008

### シナリオ AUTH-009: セッションタイムアウト境界
- **対象領域**: セッショントークン寿命
- **脅威仮説**: 長寿命セッションによるリスク拡大
- **前提**: トークン設定が[nyx-core/src/config](../../nyx-core/src)の推奨値
- **検証手順**: UI/API双方で閾値近辺アクセスを連続実行し自動再認証を観測
- **期待アウトカム**: timeout寸前に継続アクセスが再認証要求へ遷移
- **メトリクス/SLI**: `session.timeout_breach` = 0
- **失敗時対応**: [runbooks/session-reset.md](../../runbooks/session-reset.md)で強制失効
- **参照Runbook**: `RB-SES-001`
- **トレーサビリティ**: REQ-SES-004, RSK-SES-002

### シナリオ AUTH-010: リフレッシュトークン再発行
- **対象領域**: リフレッシュトークンローテーション
- **脅威仮説**: 再発行漏れによるトークン再利用
- **前提**: `nyx-cli`最新版に更新
- **検証手順**: 連続リフレッシュリクエストを発行し旧トークンが直ちに無効化されることを確認
- **期待アウトカム**: 旧トークン使用時に`TOKEN_REUSED`で拒否
- **メトリクス/SLI**: `refresh.rotation_success` ≥ 99.99%
- **失敗時対応**: [runbooks/token-rotation.md](../../runbooks/token-rotation.md)
- **参照Runbook**: `RB-TKN-003`
- **トレーサビリティ**: REQ-AUTH-036, ADR-TOKEN-001

### シナリオ AUTH-011: セッションハイジャック検出
- **対象領域**: セッション整合性監視
- **脅威仮説**: セッションID流出による乗っ取り
- **前提**: 位置情報/デバイス指紋が監査ログに記録される設定
- **検証手順**: 異なるIPで同セッション使用を試み異常検知イベントを確認
- **期待アウトカム**: アラート`SESSION_ANOMALY`が30秒以内に発火
- **メトリクス/SLI**: `session.anomaly_detection_latency` ≤ 30秒
- **失敗時対応**: [runbooks/session-compromise.md](../../runbooks/session-compromise.md)
- **参照Runbook**: `RB-SES-005`
- **トレーサビリティ**: RSK-SEC-004, AUD-AUTH-002

### シナリオ AUTH-012: Auditイベント完全性
- **対象領域**: 監査ログ署名と配送
- **脅威仮説**: 監査ログ改竄
- **前提**: [security/audit.md](./audit.md)で定義するハッシュチェーン有効
- **検証手順**: ログ配送遅延を注入しチェーン整合性検証と欠落検知を確認
- **期待アウトカム**: 欠落時に`AUDIT_CHAIN_BROKEN`アラート
- **メトリクス/SLI**: `audit.chain_integrity` = 100%
- **失敗時対応**: [runbooks/audit-gap.md](../../runbooks/audit-gap.md)
- **参照Runbook**: `RB-AUD-004`
- **トレーサビリティ**: REQ-AUD-001, RSK-COM-002

### シナリオ AUTH-013: フェデレーションIdP障害
- **対象領域**: フェデレーション冗長性
- **脅威仮説**: 外部IdP障害で認証不能
- **前提**: 代替IdPへのフェイルオーバー設定([architecture/overview.md](../architecture/overview.md))
- **検証手順**: プライマリIdP停止を模擬しフェイルオーバー挙動と監査記録を確認
- **期待アウトカム**: 代替IdPで認証継続し、ユーザー通知が実施される
- **メトリクス/SLI**: `idp.failover_time_seconds` ≤ 60
- **失敗時対応**: [runbooks/idp-failover.md](../../runbooks/idp-failover.md)
- **参照Runbook**: `RB-IDP-002`
- **トレーサビリティ**: ADR-AUTH-003, RSK-IDP-001

### シナリオ AUTH-014: サービスアカウントローテーション
- **対象領域**: 非対話型クライアント証明書
- **脅威仮説**: 長期間固定クレデンシャル
- **前提**: IAMカタログ([docs/architecture/dependencies.md](../architecture/dependencies.md))最新
- **検証手順**: サービスアカウントキーをローテーションし古いキー拒否と監査ログを確認
- **期待アウトカム**: 旧キー使用でHTTP 401、監査に`SERVICE_ACCOUNT_ROTATION`
- **メトリクス/SLI**: `service_account.rotation_interval_days` ≤ 30
- **失敗時対応**: [runbooks/service-account.md](../../runbooks/service-account.md)
- **参照Runbook**: `RB-IAM-006`
- **トレーサビリティ**: REQ-IAM-004, AUD-SEC-008

### シナリオ AUTH-015: mTLS失敗時フォールバック禁止
- **対象領域**: フォールバック経路制御
- **脅威仮説**: mTLS失敗時に平文接続へ降格
- **前提**: ネットワークポリシーが[deployment/network-policies.md](../deployment/network-policies.md)に準拠
- **検証手順**: mTLSを意図的に失敗させフォールバック試行が拒否されることを確認
- **期待アウトカム**: HTTP 525または接続拒否、フォールバック試行なし
- **メトリクス/SLI**: `mtls.fallback_attempts` = 0
- **失敗時対応**: [runbooks/mtls-hardening.md](../../runbooks/mtls-hardening.md)
- **参照Runbook**: `RB-NET-003`
- **トレーサビリティ**: RSK-NET-005, REQ-CRYPTO-012

### シナリオ AUTH-016: セッションブラックリスト試験
- **対象領域**: セッション失効リスト
- **脅威仮説**: ブラックリスト伝搬遅延
- **前提**: Redisクラスタ監視が稼働([nyx-telemetry](../../nyx-telemetry/Cargo.toml))
- **検証手順**: セッション強制失効を発行しAPI応答が即時401になるか確認
- **期待アウトカム**: 失効通知から15秒以内に無効化
- **メトリクス/SLI**: `session.revocation_latency_seconds` ≤ 15
- **失敗時対応**: [runbooks/session-revocation.md](../../runbooks/session-revocation.md)
- **参照Runbook**: `RB-SES-004`
- **トレーサビリティ**: REQ-SES-006, RSK-SES-005

### シナリオ AUTH-017: OAuth Scopes整合性
- **対象領域**: スコープ/クレーム検証
- **脅威仮説**: 過大スコープ付与
- **前提**: [docs/architecture/interfaces.md](../architecture/interfaces.md)の契約テーブル更新済み
- **検証手順**: 不正スコープ要求を送信しIdP拒否とAPI側での再検証を確認
- **期待アウトカム**: `INVALID_SCOPE`エラーと監査イベント
- **メトリクス/SLI**: `scope.mismatch_rate` ≤ 0.01%
- **失敗時対応**: [runbooks/scope-remediation.md](../../runbooks/scope-remediation.md)
- **参照Runbook**: `RB-SCOPE-001`
- **トレーサビリティ**: REQ-AUTH-045, ADR-AUTH-012

### シナリオ AUTH-018: 管理者緊急アクセス手順
- **対象領域**: Break-glassアカウント利用管理
- **脅威仮説**: 緊急アクセス乱用
- **前提**: 緊急アカウントが`hardware key + short-lived token`
- **検証手順**: 緊急アクセスをシミュレートし二人承認、事後レビューを確認
- **期待アウトカム**: 利用時に監査ログと即時通知、使用後自動失効
- **メトリクス/SLI**: `breakglass.audit_completion_time` ≤ 24時間
- **失敗時対応**: [runbooks/breakglass-review.md](../../runbooks/breakglass-review.md)
- **参照Runbook**: `RB-BG-001`
- **トレーサビリティ**: RSK-GOV-002, AUD-SEC-009

### シナリオ AUTH-019: 自動ポリシー撤回
- **対象領域**: ポリシー自動無効化
- **脅威仮説**: 期限切れポリシーの残存
- **前提**: ポリシーに`expiry`フィールドが設定
- **検証手順**: 有効期限切れポリシーを保有し自動撤回処理と通知を確認
- **期待アウトカム**: 期限後に即座にポリシー削除、監査記録
- **メトリクス/SLI**: `policy.expiry_enforced_rate` = 100%
- **失敗時対応**: [runbooks/policy-expiry.md](../../runbooks/policy-expiry.md)
- **参照Runbook**: `RB-POL-004`
- **トレーサビリティ**: REQ-POL-005, ADR-OPA-004

### シナリオ AUTH-020: 監査ログ配送遅延
- **対象領域**: ログストリーム遅延検知
- **脅威仮説**: 配送遅延によるリアルタイム検出不全
- **前提**: Grafana Loki/Tempoが[docs/architecture/dataflow.md](../architecture/dataflow.md)に沿って構成
- **検証手順**: 人為的遅延を挿入し遅延アラート発報を確認
- **期待アウトカム**: 60秒遅延で`AUDIT_DELIVERY_DELAY`アラート
- **メトリクス/SLI**: `audit.delivery_latency_p95` ≤ 45秒
- **失敗時対応**: [runbooks/log-delivery.md](../../runbooks/log-delivery.md)
- **参照Runbook**: `RB-LOG-002`
- **トレーサビリティ**: RSK-OBS-003, AUD-SEC-004

### シナリオ AUTH-021: 多地域認証遅延
- **対象領域**: 地理分散認証
- **脅威仮説**: 遅延増加によるタイムアウトと再試行増加
- **前提**: GeoDNS構成が最新
- **検証手順**: 異地域からのログイン試験で遅延指標とセッション成功率を収集
- **期待アウトカム**: 遅延<200msで成功率維持
- **メトリクス/SLI**: `auth.latency_p95` ≤ 200ms
- **失敗時対応**: [runbooks/geo-routing.md](../../runbooks/geo-routing.md)
- **参照Runbook**: `RB-NET-005`
- **トレーサビリティ**: REQ-AUTH-054, PERF-SLO-002

### シナリオ AUTH-022: ハードウェアキー更新
- **対象領域**: FIDO2/YubiKey管理
- **脅威仮説**: ハードウェアキーの期限切れ・紛失
- **前提**: 管理者ユーザーに代替キー登録済み
- **検証手順**: ハードウェアキー更新プロセスを通し旧キー無効化と新キー登録を確認
- **期待アウトカム**: 無効化後に旧キー利用不可、新キーのみ許可
- **メトリクス/SLI**: `mfa.hw_key_rotation_completion` ≤ 48時間
- **失敗時対応**: [runbooks/hw-key-rotation.md](../../runbooks/hw-key-rotation.md)
- **参照Runbook**: `RB-MFA-004`
- **トレーサビリティ**: RSK-MFA-003, ADR-SEC-015

### シナリオ AUTH-023: CLI認証フロー検証
- **対象領域**: `nyx-cli`デバイスコードフロー
- **脅威仮説**: CLI認証結果とUI権限差異
- **前提**: CLIが最新、[nyx-cli/tests](../../nyx-cli/tests)が成功
- **検証手順**: デバイスコードフローで認証し権限クレームをUIと比較
- **期待アウトカム**: CLIトークンとUIトークンでクレーム一致
- **メトリクス/SLI**: `cli.auth_claim_parity` = 100%
- **失敗時対応**: [runbooks/cli-auth.md](../../runbooks/cli-auth.md)
- **参照Runbook**: `RB-CLI-002`
- **トレーサビリティ**: REQ-CLI-003, AUD-SEC-012

### シナリオ AUTH-024: SDKトークンハンドリング
- **対象領域**: SDK内トークンキャッシュ
- **脅威仮説**: トークン再利用による権限漏洩
- **前提**: SDKバージョン固定、[nyx-sdk/tests](../../nyx-sdk/tests)がカバレッジ100%
- **検証手順**: トークンローリング時のキャッシュ破棄を確認
- **期待アウトカム**: 無効トークン使用時にSDKが自動更新
- **メトリクス/SLI**: `sdk.token_stale_usage` = 0
- **失敗時対応**: [runbooks/sdk-token.md](../../runbooks/sdk-token.md)
- **参照Runbook**: `RB-SDK-001`
- **トレーサビリティ**: REQ-SDK-006, ADR-SDK-002

### シナリオ AUTH-025: 多要素疲労攻撃対策
- **対象領域**: MFAプッシュ通知耐性
- **脅威仮説**: MFA疲労による承認誤操作
- **前提**: プッシュ通知レート制限構成済み
- **検証手順**: 連続プッシュを模擬しレート制限とユーザー通知を確認
- **期待アウトカム**: 連続試行がブロックされ管理者にアラート
- **メトリクス/SLI**: `mfa.push_rate_limit_trigger` ≥ 1回/シミュレーション
- **失敗時対応**: [runbooks/mfa-fatigue.md](../../runbooks/mfa-fatigue.md)
- **参照Runbook**: `RB-MFA-006`
- **トレーサビリティ**: RSK-MFA-005, AUD-SEC-015

### シナリオ AUTH-026: セッション再認証閾値
- **対象領域**: 高感度操作前の再認証
- **脅威仮説**: 継続セッションでのハイリスク操作
- **前提**: 高感度操作がタグ付けされている
- **検証手順**: 設定変更時に再認証促進が発動するか確認
- **期待アウトカム**: 高感度操作で追加認証が必須
- **メトリクス/SLI**: `session.reauth_prompt_rate` = 100%
- **失敗時対応**: [runbooks/session-reauth.md](../../runbooks/session-reauth.md)
- **参照Runbook**: `RB-SES-007`
- **トレーサビリティ**: REQ-AUTH-060, ADR-SEC-020

### シナリオ AUTH-027: RBACロール削除影響
- **対象領域**: ロール削除の波及確認
- **脅威仮説**: ロール削除で孤立セッションが残存
- **前提**: 影響範囲分析ツールが[tools/rbac-impact](../../tools)に配置
- **検証手順**: ステージングでロール削除を実行し影響分析とセッション無効化を確認
- **期待アウトカム**: 対象セッションが即時終了、影響レポート生成
- **メトリクス/SLI**: `rbac.role_delete_orphan_sessions` = 0
- **失敗時対応**: [runbooks/rbac-cleanup.md](../../runbooks/rbac-cleanup.md)
- **参照Runbook**: `RB-RBAC-010`
- **トレーサビリティ**: REQ-RBAC-014, AUD-SEC-020

### シナリオ AUTH-028: ABAC属性キャッシング
- **対象領域**: 属性キャッシュ有効期限
- **脅威仮説**: 古い属性による誤許可
- **前提**: 属性ソースが[nyx-telemetry](../../nyx-telemetry)で監視
- **検証手順**: 属性更新後直ちにアクセス試行しキャッシュ更新を確認
- **期待アウトカム**: 直後アクセスで最新属性が反映
- **メトリクス/SLI**: `abac.attribute_refresh_latency` ≤ 30秒
- **失敗時対応**: [runbooks/abac-cache.md](../../runbooks/abac-cache.md)
- **参照Runbook**: `RB-ABAC-004`
- **トレーサビリティ**: REQ-ABAC-010, RSK-ABAC-003

### シナリオ AUTH-029: OPAルール競合検知
- **対象領域**: ポリシー競合解析
- **脅威仮説**: ルール競合による許可/拒否の揺らぎ
- **前提**: ポリシーテストが[formal/test_configurations.py](../../formal/test_configurations.py)と連携
- **検証手順**: 意図的競合ルール投入でCI検出精度を確認
- **期待アウトカム**: CIで競合検知、デプロイブロック
- **メトリクス/SLI**: `opa.policy_conflict_detected` = 100%
- **失敗時対応**: [runbooks/policy-conflict.md](../../runbooks/policy-conflict.md)
- **参照Runbook**: `RB-OPA-005`
- **トレーサビリティ**: ADR-OPA-006, AUD-SEC-023

### シナリオ AUTH-030: Audit不可視イベント監査
- **対象領域**: 監査不可視操作の検出
- **脅威仮説**: 監査ログ対象外操作の潜在化
- **前提**: 操作カタログが[docs/architecture/interfaces.md](../architecture/interfaces.md)に反映
- **検証手順**: 非監査対象操作を列挙し監査範囲チェックリストを更新
- **期待アウトカム**: すべての操作に監査可視化が定義
- **メトリクス/SLI**: `audit.untracked_actions` = 0
- **失敗時対応**: [runbooks/audit-coverage.md](../../runbooks/audit-coverage.md)
- **参照Runbook**: `RB-AUD-006`
- **トレーサビリティ**: AUD-SEC-030, REQ-AUD-010

### シナリオ AUTH-031: IdP証明書期限切れ警告
- **対象領域**: フェデレーションメタデータ監視
- **脅威仮説**: 証明書期限切れによる認証失敗
- **前提**: 証明書有効期限監視がGrafanaダッシュボード化
- **検証手順**: テスト環境で短期証明書を導入し警告閾値トリガーを確認
- **期待アウトカム**: 14日前警告、7日前クリティカル
- **メトリクス/SLI**: `idp.certificate_warning_lead_days` ≥ 14
- **失敗時対応**: [runbooks/idp-certificate.md](../../runbooks/idp-certificate.md)
- **参照Runbook**: `RB-IDP-004`
- **トレーサビリティ**: RSK-IDP-004, ADR-AUTH-018

### シナリオ AUTH-032: セッションキー同期失敗
- **対象領域**: セッションキー分散ストレージ整合性
- **脅威仮説**: キー同期失敗で復号不能
- **前提**: 分散KMSが[docs/architecture/dataflow.md](../architecture/dataflow.md)整合
- **検証手順**: レプリカ停止時のキー同期エラーを観測
- **期待アウトカム**: 冗長KMSへフェイルオーバーし継続
- **メトリクス/SLI**: `session.key_sync_failure_rate` ≤ 0.001%
- **失敗時対応**: [runbooks/kms-failover.md](../../runbooks/kms-failover.md)
- **参照Runbook**: `RB-KMS-003`
- **トレーサビリティ**: REQ-CRYPTO-015, RSK-KMS-002

### シナリオ AUTH-033: 高感度リソースアクセス
- **対象領域**: High Value Assetアクセス
- **脅威仮説**: 高感度リソースへの不正アクセス
- **前提**: リソース分類が[docs/compliance_ci_integration.md](../compliance_ci_integration.md)に登録
- **検証手順**: High Value Assetアクセス時のABAC条件と監査記録を確認
- **期待アウトカム**: 追加承認とレート制限が適用
- **メトリクス/SLI**: `hva.access_control_enforced` = 100%
- **失敗時対応**: [runbooks/hva-incident.md](../../runbooks/hva-incident.md)
- **参照Runbook**: `RB-HVA-002`
- **トレーサビリティ**: REQ-ABAC-014, AUD-SEC-034

### シナリオ AUTH-034: APIトークン権限縮小
- **対象領域**: APIトークン権限削減
- **脅威仮説**: トークン肥大化による攻撃面増大
- **前提**: トークン権限レビューが月次実施
- **検証手順**: 過大権限トークンを特定し権限縮小後に動作確認
- **期待アウトカム**: 必要最小権限で正常稼働
- **メトリクス/SLI**: `token.scope_reduction_success` ≥ 95%
- **失敗時対応**: [runbooks/token-scope.md](../../runbooks/token-scope.md)
- **参照Runbook**: `RB-TKN-005`
- **トレーサビリティ**: RSK-API-005, ADR-AUTH-020

### シナリオ AUTH-035: セッション継続監査
- **対象領域**: 長期セッション監査
- **脅威仮説**: 長期セッションからの侵害継続
- **前提**: セッションメタデータが監査ログと突合可能
- **検証手順**: 30日継続セッションを抽出し再認証記録を確認
- **期待アウトカム**: ポリシーで定めた再認証イベントが存在
- **メトリクス/SLI**: `session.long_lived_without_review` = 0
- **失敗時対応**: [runbooks/session-audit.md](../../runbooks/session-audit.md)
- **参照Runbook**: `RB-SES-010`
- **トレーサビリティ**: REQ-SES-012, AUD-SEC-040

### シナリオ AUTH-036: フェデレーションメタデータ更新
- **対象領域**: SAML/OIDCメタデータローテーション
- **脅威仮説**: 古いメタデータで署名検証失敗
- **前提**: 自動フェッチ設定([deployment/ci-cd.md](../deployment/ci-cd.md))
- **検証手順**: メタデータ更新時の自動反映と署名検証を確認
- **期待アウトカム**: 更新後もサービス中断なし
- **メトリクス/SLI**: `idp.metadata_refresh_latency` ≤ 5分
- **失敗時対応**: [runbooks/idp-metadata.md](../../runbooks/idp-metadata.md)
- **参照Runbook**: `RB-IDP-006`
- **トレーサビリティ**: ADR-AUTH-025, RSK-IDP-007

### シナリオ AUTH-037: アクセストークン暗号強度評価
- **対象領域**: JWT署名アルゴリズム
- **脅威仮説**: 弱いアルゴリズム`none`/`HS256`の混入
- **前提**: `alg`ホワイトリストが`RS512`のみ
- **検証手順**: 弱い`alg`を設定したトークン送信し拒否を確認
- **期待アウトカム**: `UNSUPPORTED_ALG`エラー
- **メトリクス/SLI**: `token.unsupported_alg_rejection` = 100%
- **失敗時対応**: [runbooks/token-alg.md](../../runbooks/token-alg.md)
- **参照Runbook**: `RB-TKN-007`
- **トレーサビリティ**: REQ-CRYPTO-020, RSK-TOKEN-002

### シナリオ AUTH-038: Policyロールバック検証
- **対象領域**: ポリシーバージョン管理
- **脅威仮説**: 不良ポリシー適用からの回復失敗
- **前提**: gitタグとOPAバンドルが同期
- **検証手順**: 意図的に誤ポリシーを適用しロールバック手順を確認
- **期待アウトカム**: 5分以内に前バージョンへ復帰
- **メトリクス/SLI**: `policy.rollback_time_minutes` ≤ 5
- **失敗時対応**: [runbooks/policy-rollback.md](../../runbooks/policy-rollback.md)
- **参照Runbook**: `RB-OPA-007`
- **トレーサビリティ**: ADR-OPA-009, AUD-SEC-045

### シナリオ AUTH-039: スタッフ離任時アクセス遮断
- **対象領域**: オフボーディング
- **脅威仮説**: 離任者がアクセス継続
- **前提**: HRフィードがリアルタイム
- **検証手順**: 離任イベント発火でアカウント失効と監査確認
- **期待アウトカム**: 15分以内に全トークン無効、権限剥奪
- **メトリクス/SLI**: `offboarding.revocation_latency_minutes` ≤ 15
- **失敗時対応**: [runbooks/offboarding.md](../../runbooks/offboarding.md)
- **参照Runbook**: `RB-HR-003`
- **トレーサビリティ**: REQ-IAM-009, AUD-SEC-050

### シナリオ AUTH-040: 分散監査ログ整合性
- **対象領域**: マルチリージョン監査ログ
- **脅威仮説**: リージョン間整合性欠如
- **前提**: クロスリージョンレプリケーション有効
- **検証手順**: 各リージョンで同一イベント発生時刻を比較
- **期待アウトカム**: タイムスタンプ差異<5秒
- **メトリクス/SLI**: `audit.cross_region_skew_seconds` ≤ 5
- **失敗時対応**: [runbooks/audit-multiregion.md](../../runbooks/audit-multiregion.md)
- **参照Runbook**: `RB-AUD-008`
- **トレーサビリティ**: RSK-OBS-006, ADR-LOG-002

### シナリオ AUTH-041: SSOダウン時代替経路
- **対象領域**: SSOフォールバック
- **脅威仮説**: SSO停止で作業不能
- **前提**: フェイルセーフローカル認証ポリシー定義
- **検証手順**: SSO停止シミュレーションで代替認証起動を確認
- **期待アウトカム**: 最小権限で業務継続、監査記録
- **メトリクス/SLI**: `sso.fallback_activation_time` ≤ 10分
- **失敗時対応**: [runbooks/sso-fallback.md](../../runbooks/sso-fallback.md)
- **参照Runbook**: `RB-SSO-002`
- **トレーサビリティ**: RSK-SSO-002, ADR-AUTH-030

### シナリオ AUTH-042: セッションレート制限逸脱
- **対象領域**: 認証レート制限
- **脅威仮説**: ブルートフォース攻撃未検知
- **前提**: レート制限ルールがWAFと同期
- **検証手順**: 高頻度ログイン試行で制限発火とアラート確認
- **期待アウトカム**: HTTP 429、アラート`AUTH_RATE_LIMIT`
- **メトリクス/SLI**: `auth.rate_limit_block_efficiency` ≥ 99%
- **失敗時対応**: [runbooks/auth-rate-limit.md](../../runbooks/auth-rate-limit.md)
- **参照Runbook**: `RB-WAF-001`
- **トレーサビリティ**: RSK-AUTH-009, AUD-SEC-055

### シナリオ AUTH-043: デバイス紛失対応
- **対象領域**: 紛失/盗難端末処理
- **脅威仮説**: 紛失端末からの認証試行
- **前提**: MDM統合が稼働
- **検証手順**: 紛失報告→MDM隔離→トークン失効を一連確認
- **期待アウトカム**: 30分以内にデバイス認証無効
- **メトリクス/SLI**: `device.loss_response_time_minutes` ≤ 30
- **失敗時対応**: [runbooks/device-loss.md](../../runbooks/device-loss.md)
- **参照Runbook**: `RB-DEV-006`
- **トレーサビリティ**: RSK-MBL-006, ADR-SEC-034

### シナリオ AUTH-044: APIキー最小権限確認
- **対象領域**: APIキーガバナンス
- **脅威仮説**: APIキーに過大権限付与
- **前提**: APIキー台帳が最新
- **検証手順**: 全APIキーの権限棚卸と不要権限削除
- **期待アウトカム**: 最小権限化計画完遂
- **メトリクス/SLI**: `apikey.least_privilege_compliance` ≥ 98%
- **失敗時対応**: [runbooks/api-key-review.md](../../runbooks/api-key-review.md)
- **参照Runbook**: `RB-API-002`
- **トレーサビリティ**: REQ-API-004, AUD-SEC-060

### シナリオ AUTH-045: RBAC監査証跡整合
- **対象領域**: RBAC変更監査
- **脅威仮説**: RBAC変更記録不足
- **前提**: 監査システムとRBACストアが同期
- **検証手順**: RBAC変更イベントが監査ログと一致するか検証
- **期待アウトカム**: すべての変更に監査イベント
- **メトリクス/SLI**: `rbac.audit_trail_completeness` = 100%
- **失敗時対応**: [runbooks/rbac-audit.md](../../runbooks/rbac-audit.md)
- **参照Runbook**: `RB-RBAC-012`
- **トレーサビリティ**: AUD-SEC-065, REQ-RBAC-018

### シナリオ AUTH-046: ABAC属性ソース障害
- **対象領域**: 属性ソース冗長化
- **脅威仮説**: 属性ソース停止で認可判断失敗
- **前提**: セカンダリ属性ソース定義済み
- **検証手順**: プライマリ属性ソース停止を模擬しフェイルオーバー確認
- **期待アウトカム**: 認可判断継続、アラート記録
- **メトリクス/SLI**: `abac.attribute_source_failover_time` ≤ 60秒
- **失敗時対応**: [runbooks/abac-source.md](../../runbooks/abac-source.md)
- **参照Runbook**: `RB-ABAC-006`
- **トレーサビリティ**: RSK-ABAC-007, ADR-ABAC-006

### シナリオ AUTH-047: OPAデプロイロールアウト
- **対象領域**: カナリアデプロイ
- **脅威仮説**: ポリシー更新による即時障害
- **前提**: カナリア比率設定が[deployment/ci-cd.md](../deployment/ci-cd.md)に準拠
- **検証手順**: カナリア→全体デプロイでメトリクス監視
- **期待アウトカム**: カナリア段階で異常検知時ロールバック
- **メトリクス/SLI**: `opa.canary_abort_rate` ≥ 95%
- **失敗時対応**: [runbooks/policy-canary.md](../../runbooks/policy-canary.md)
- **参照Runbook**: `RB-OPA-010`
- **トレーサビリティ**: ADR-OPA-011, AUD-SEC-070

### シナリオ AUTH-048: セッションメタデータ完全性
- **対象領域**: セッションメタデータ暗号化
- **脅威仮説**: 改竄によるセッション乗っ取り
- **前提**: メタデータがHMAC署名
- **検証手順**: 署名改竄セッションを投入し検知
- **期待アウトカム**: `SESSION_METADATA_TAMPERED`エラー
- **メトリクス/SLI**: `session.metadata_tamper_detected` = 100%
- **失敗時対応**: [runbooks/session-metadata.md](../../runbooks/session-metadata.md)
- **参照Runbook**: `RB-SES-012`
- **トレーサビリティ**: REQ-SES-020, AUD-SEC-075

### シナリオ AUTH-049: 監査アラートノイズ削減
- **対象領域**: 監査アラートチューニング
- **脅威仮説**: ノイズ多発で重大イベント見逃し
- **前提**: アラート分類が[docs/telemetry.md](../telemetry.md)に定義
- **検証手順**: ノイズアラートを分析し閾値調整後の偽陽性率を測定
- **期待アウトカム**: 偽陽性率が半減
- **メトリクス/SLI**: `audit.alert_false_positive_rate` ≤ 5%
- **失敗時対応**: [runbooks/audit-alert-tuning.md](../../runbooks/audit-alert-tuning.md)
- **参照Runbook**: `RB-AUD-010`
- **トレーサビリティ**: AUD-SEC-080, RSK-OBS-010

### シナリオ AUTH-050: 緊急時アクセス放棄確認
- **対象領域**: 緊急アクセス後の退出
- **脅威仮説**: 緊急アクセス終了後のセッション継続
- **前提**: Break-glassセッションが短寿命設定
- **検証手順**: 緊急アクセス後にセッションが自動終了するか確認
- **期待アウトカム**: 操作完了後5分でセッション失効
- **メトリクス/SLI**: `breakglass.session_termination_time` ≤ 5分
- **失敗時対応**: [runbooks/breakglass-review.md](../../runbooks/breakglass-review.md)
- **参照Runbook**: `RB-BG-002`
- **トレーサビリティ**: AUD-SEC-085, REQ-AUTH-070

### シナリオ運用サマリ
シナリオカタログは運用チームの年間計画に組み込まれ、SRE・セキュリティ・プロダクト各組織の役割を明確化する。

| シナリオクラスタ | 想定頻度 | 実行責任 (R) | 承認 (A) | 協力 (C) | 通知 (I) |
|------------------|----------|---------------|----------|----------|----------|
| OIDC/OAuthフロー (AUTH-001〜AUTH-025) | 月次CI/CDサイクル | セキュリティエンジニアリング | プロダクトセキュリティリード | QA, DevOps | 監査チーム |
| ポリシー/OPA関連 (AUTH-008, AUTH-019, AUTH-029, AUTH-038, AUTH-047) | リリース毎 | プラットフォームSRE | CISO代理 | セキュリティエンジニアリング | リリースマネジメント |
| セッション/デバイス (AUTH-005, AUTH-009〜AUTH-016, AUTH-035, AUTH-048) | 週次 | プラットフォームSRE | セキュリティエンジニアリング | MDM, IT | 事業オーナー |
| ブレークグラス/緊急対応 (AUTH-018, AUTH-039, AUTH-041, AUTH-050) | 半期/演習毎 | セキュリティオペレーション | CISO | HR, Legal | 取締役会 |

- **自動化統合**: AUTH-001〜AUTH-044は[testing/integration-tests.md](../testing/integration-tests.md)のCIステージで自動化され、失敗時は`nyx-ci`パイプラインがブロックされる。
- **手動演習**: 緊急アクセス系シナリオは[docs/quickstart-ubuntu-k8s.md](../quickstart-ubuntu-k8s.md)の演習手順と連動し、演習後は[notes/decision-log.md](../notes/decision-log.md)へレビュー結果を記録する。
- **成果物保管**: 実施エビデンスは`/evidence/security/auth/YYYYMM/`に保存し、[compliance_ci_integration.md](../compliance_ci_integration.md)のDoDチェックで参照する。
- **インシデント連携**: 異常検知時は[runbooks/incident-response.md](../../runbooks/incident-response.md)へ遷移し、対応完了後`AUTH-IR`タグで事後レビューを整理する。

### 保証メトリクスダッシュボード
Grafanaダッシュボード`SEC-ASSURANCE-01`で以下のKPI/SLIを監視し、[docs/performance/scalability.md](../performance/scalability.md)の容量計画と整合させる。閾値逸脱は`pager-duty:security`へ通知される。

| メトリクスID | 説明 | 収集ソース | 目標値 (SLO) | アラート閾値 | 関連シナリオ |
|--------------|------|------------|--------------|---------------|---------------|
| AUTH-SLI-01 | PKCE検証失敗率 | `auth.pkce_validation_failure` (Tempo) | 0% | >0.01% (5分) | AUTH-001, AUTH-002 |
| AUTH-SLI-05 | mTLSハンドシェイク成功率 | `mtls.handshake_success_rate` (Prometheus) | ≥99.9% | <99.5% (10分) | AUTH-003, AUTH-015 |
| AUTH-SLI-09 | セッション失効伝搬遅延p95 | `session.revocation_latency_seconds` | ≤15秒 | >30秒 (連続3点) | AUTH-016 |
| AUTH-SLI-12 | OPA競合検出ブロック率 | `opa.policy_conflict_detected` | 100% | <100% (単発) | AUTH-008, AUTH-029 |
| AUTH-SLI-18 | ブレークグラス事後レビュー完了率 | `breakglass.audit_completion_time` | 100% within 24h | 未完了>24h | AUTH-018, AUTH-050 |
| AUTH-SLI-22 | APIキー最小権限準拠率 | `apikey.least_privilege_compliance` | ≥98% | <95% (週次) | AUTH-044 |

- **メトリクス追加手順**: 新規KPIは[telemetry.md](../telemetry.md)の`SECURITY_METRIC`テンプレートで宣言し、[deployment/ci-cd.md](../deployment/ci-cd.md)のパイプラインでPrometheus Ruleを登録する。
- **SLOレビュー**: 四半期レビューで[compliance_ci_integration.md](../compliance_ci_integration.md)の`SECURITY-SLO-BOARD`へ最新指標を転記し、役員向けレポートにトレンド分析を反映する。
- **監査証跡**: ダッシュボードスナップショットを`/evidence/security/dashboards/`へ出力し、監査人がAUTH-SLIメトリクスを追跡再現できるようにする。

### 継続的改善バックログ
継続的改善アイテムは`JIRA:SEC-ROADMAP`で管理し、四半期ポートフォリオレビューで優先順位を評価する。

1. **AUTH-BL-001**: IdPフェイルオーバー自動化をTerraform module (`infra/idp-failover/`) に統合。関連: AUTH-013, [deployment/network-policies.md](../deployment/network-policies.md)。
2. **AUTH-BL-004**: セッションハイジャック検出を機械学習フィードへ拡張し、[nyx-telemetry](../../nyx-telemetry)に特徴量を追加。関連: AUTH-011, AUTH-035。
3. **AUTH-BL-007**: デバイス証明書ローテーションUXを`nyx-mobile-ffi`ガイドに追記し、ユーザー教育資料[quickstart-ubuntu-k8s.md](../quickstart-ubuntu-k8s.md)へ反映。
4. **AUTH-BL-010**: ポリシー競合検出CIを[formal/run_model_checking.py](../../formal/run_model_checking.py)に組み込みフォーマル検証カバレッジを拡大。関連: AUTH-029。
5. **AUTH-BL-012**: ブレークグラス対応のレッドチーム演習を半期で実施し、結果を[security/vulnerability.md](./vulnerability.md)に追記。
6. **AUTH-BL-015**: APIキー棚卸し作業を自動レポート化し、[scripts/](../../scripts)配下にレポートジェネレーターを追加。
7. **AUTH-BL-018**: Auditノイズ削減実験をA/Bテストし、`audit.alert_false_positive_rate`改善値を[telemetry.md](../telemetry.md)に反映。
8. **AUTH-BL-020**: 緊急アクセス放棄確認を`nyx-cli` self-testに追加し、利用者がAUTH-050準拠をセルフチェック可能にする。
9. **AUTH-BL-024**: SSO代替経路のChaos実験を[performance/specs.md](../performance/specs.md)へ組み込み、障害注入手順をドキュメント化。
10. **AUTH-BL-030**: 監査イベント不可視領域(AUTH-030)のカバレッジ監査を自動化し、`compliance/coverage-report.md`（新規作成予定）で可視化。

完了した項目は`SEC-ASSURANCE` OKRに紐付け、DoD証跡として本ドキュメントの該当セクションを更新する。

## 関連ドキュメント
- [security/encryption.md](./encryption.md)
- [security/vulnerability.md](./vulnerability.md)
- [architecture/interfaces.md](../architecture/interfaces.md)
- [testing/e2e-tests.md](../testing/e2e-tests.md)

> **宣言**: 本章は実装コードを含まず、C/C++依存要素を採用しない。