# docs/requirements.md

> **遵守バッジ** : :no_entry: 実装コード非出力 / :no_entry_sign: C/C++依存禁止（本書全体で厳守）

## 目次
- [ドキュメント目的](#ドキュメント目的)
- [前提と参照](#前提と参照)
- [ペルソナ](#ペルソナ)
- [ユースケース一覧](#ユースケース一覧)
- [機能要件](#機能要件)
- [非機能要件](#非機能要件)
- [環境・制約](#環境制約)
- [遵守事項と禁止事項](#遵守事項と禁止事項)
- [スコープ外項目](#スコープ外項目)
- [トレーサビリティ方針](#トレーサビリティ方針)
- [用語集](#用語集)

## ドキュメント目的
Nyxプロジェクトの機能・非機能要件を体系的に管理し、設計（[architecture](./architecture/overview.md)）およびテスト計画（[testing](./testing/unit-tests.md) ほか）とのトレーサビリティを確立する。すべての要件は識別子`REQ-<カテゴリ>-<番号>`で管理し、変更はADR ([notes/decision-log.md](./notes/decision-log.md)) を通じて承認する。

## 前提と参照
- 企画背景とビジョン: [docs/README.md](./README.md)
- アーキテクチャ構造: [architecture/overview.md](./architecture/overview.md)
- セキュリティモデル: [security/vulnerability.md](./security/vulnerability.md)
- 性能ターゲット: [performance/scalability.md](./performance/scalability.md)
- 運用ポリシー: [deployment/infrastructure.md](./deployment/infrastructure.md)

## ペルソナ
| ID | 説明 | 主な課題 | 参照要件 |
|----|------|----------|-----------|
| P1 | 規制遵守責任者 (RegSec Officer) | 監査可能性と法規制順守 | REQ-FUN-020, REQ-NFR-040 |
| P2 | モバイルアプリ開発者 | 遅延と省電力バランス | REQ-FUN-030, REQ-NFR-010 |
| P3 | 匿名性を重視する市民記者 | 追跡困難な通信 | REQ-FUN-010, REQ-NFR-020 |
| P4 | SRE/プラットフォームエンジニア | 高可用性と障害対応 | REQ-NFR-030, REQ-NFR-050 |

## ユースケース一覧
| UC ID | タイトル | 概要 | 主体 | 成功条件 |
|-------|----------|------|------|----------|
| UC-01 | 匿名チャットセッション確立 | クライアントがNyxネットワーク経由でメッセージングを開始する | P2, P3 | 0-RTT接続成功、匿名性メトリクス満たす |
| UC-02 | モバイルハンドオーバ | モバイル端末がネットワーク切替後もセッションを維持 | P2 | 接続再確立 < 2秒、データ損失 0 |
| UC-03 | 規制監査レポート生成 | 監査人がアクセスログと暗号状態を確認 | P1 | 監査ログ改ざん不能、アクセス権制御適用 |
| UC-04 | 自動ロールバック実行 | SREがCI/CDからロールバックを指示 | P4 | サービス停止 < 5分、整合性担保 |

## 機能要件
### セッション・ストリーミング
- **REQ-FUN-010**: クライアントは0-RTTハンドシェイクを通じて匿名セッションを確立できること。成功率 \u2265 99%。
- **REQ-FUN-011**: セッション確立時、ハイブリッド鍵交換（古典+PQ）を必須とする。PQアルゴリズムは選択可能で、互換性ネゴシエーションを行う。
- **REQ-FUN-012**: ストリーム多重化により、単一セッション上で最大128の論理チャネルをサポートする。

### ルーティング・ミックスネット
- **REQ-FUN-020**: ミックス経路は3以上の独立ノードで構成し、地理的多様性を確保する。
- **REQ-FUN-021**: ルーティングはネットワーク状態に応じて30秒以内に再最適化される。
- **REQ-FUN-022**: ノード信頼度スコアの閾値管理を提供し、閾値未満は自動的に経路から除外する。

### カバー交通・オブフスケーション
- **REQ-FUN-030**: Poisson分布に基づくカバー交通を適用し、整合性検査の結果が実通信と識別不能であることを保証する。
- **REQ-FUN-031**: タイミング撹乱を実施し、外部観測者が送受信の相関を90%以上の確率で推定できないよう抑止する。
- **REQ-FUN-032**: Forward Error Correction (FEC) の適応制御を提供し、パケット損失率5%までユーザ体験を劣化させない。

### 監査・ガバナンス
- **REQ-FUN-040**: RBAC/ABACを組み合わせた権限管理を提供し、監査人・オペレーター・開発者のアクセス境界を明確化。
- **REQ-FUN-041**: すべての管理操作は不可逆な監査ログに記録し、改ざん検出機構を備える。
- **REQ-FUN-042**: 重大イベントの発生時、事後レビュー（[deployment/rollback.md](./deployment/rollback.md)）用テンプレートが自動生成される。

### API/インターフェース
- **REQ-FUN-050**: 外部APIsは言語非依存スキーマで定義し、互換性レベル (MAJOR.MINOR.PATCH) を導入する。
- **REQ-FUN-051**: SDKが公開する全エンドポイントは、アクセス制御とレート制限を実装する。詳細は[architecture/interfaces.md](./architecture/interfaces.md)。

## 非機能要件
| ID | 説明 | 指標 | 関連文書 |
|----|------|------|----------|
| **REQ-NFR-010** | パフォーマンス: P95遅延 \u2264 350ms、P99 \u2264 500ms | [performance/benchmark.md](./performance/benchmark.md) | | 
| **REQ-NFR-011** | スループット: 1ノードあたり 2000req/s を維持 | [performance/scalability.md](./performance/scalability.md) | |
| **REQ-NFR-020** | 匿名性: メタデータ解析抵抗指数(ARI) \u2265 0.9 | [security/vulnerability.md](./security/vulnerability.md) | |
| **REQ-NFR-030** | 可用性: 年間稼働率 99.95%以上、MTTR \u2264 15分 | [deployment/infrastructure.md](./deployment/infrastructure.md) | |
| **REQ-NFR-040** | コンプライアンス: GDPR/CCPA/各国規制に準拠 | [security/auth.md](./security/auth.md), [security/vulnerability.md](./security/vulnerability.md) | |
| **REQ-NFR-050** | オブザーバビリティ: 主要SLIをリアルタイム可視化 | [testing/metrics.md](./testing/metrics.md) | |
| **REQ-NFR-060** | 国際化: UI/UXはLTR/RTL双方に対応 | [ui/accessibility.md](./ui/accessibility.md) | |
| **REQ-NFR-070** | 保守性: 仕様変更に対する平均ドキュメント更新時間 48時間以内 | [notes/decision-log.md](./notes/decision-log.md) | |

## 環境・制約
- **サポートプラットフォーム**: クロスプラットフォーム（モバイル、デスクトップ、サーバ）。
- **暗号ライブラリ制約**: C/C++依存を避け、Rust/Python/JavaScript等の純粋実装またはマネージド環境を使用。必要に応じ外部KMS/SaaSで補完。
- **プロトコル互換性**: IPv6優先。QUIC互換プロトコルを採用するが、実装言語は限定しない。
- **フォーマル検証**: TLA+等の形式手法を活用、結果は[formal](../formal/README.md)配下に保存。

## 遵守事項と禁止事項
- **実装コード非出力**: 本プロジェクトの仕様書では具体的なプログラミングコードを記載しない。
- **C/C++依存禁止**: C/C++で記述されたライブラリまたはFFIに頼る設計を禁止。代替としてマネージド言語、サンドボックス、またはサービス分離を利用。
- **プライバシ保護**: 個人情報は匿名化して管理し、データ保持期間を明示。

## スコープ外項目
- ブロックチェーン統合、P2P課金システム、ハードウェア実装は現フェーズでは対象外。
- 物理層の最適化や特定ベンダー依存の機能は扱わない。

## トレーサビリティ方針
1. 要件IDは設計文書内で`REQ-...`の形で参照。
2. テストケースは[templates/test-template.md](./templates/test-template.md)に沿って要件IDをマッピング。
3. CI/CDは[deployment/ci-cd.md](./deployment/ci-cd.md)に従い、要件満足度をゲート条件として評価。
4. 変更履歴は[notes/decision-log.md](./notes/decision-log.md)のADR形式で管理。

## 用語集
| 用語 | 定義 | 参照 |
|------|------|------|
| **ARI (Anonymity Resilience Index)** | メタデータ解析に対する抵抗力を0-1で示す指標 | [performance/scalability.md](./performance/scalability.md) |
| **Hybrid Key Exchange** | 古典暗号とポスト量子暗号を併用した鍵交換方式 | [security/encryption.md](./security/encryption.md) |
| **Adaptive Cover Traffic** | ネットワーク状況に応じてダミートラフィックを調整する仕組み | [architecture/dataflow.md](./architecture/dataflow.md) |
| **RBAC / ABAC** | ロール/属性ベースアクセス制御 | [security/auth.md](./security/auth.md) |
| **SLI/SLO/SLAs** | サービスレベル指標/目標/合意 | [testing/metrics.md](./testing/metrics.md) |

> **備考**: 用語集はユースケースやアーキテクチャで共通利用し、矛盾が発生した場合は本章を正とする。