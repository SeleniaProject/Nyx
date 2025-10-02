# docs/deployment/infrastructure.md

> **遵守バッジ** : :no_entry: 実装コード非出力 / :no_entry_sign: C/C++依存禁止

## 目次
- [目的](#目的)
- [環境構成](#環境構成)
- [インフラコンポーネント](#インフラコンポーネント)
- [セキュリティ境界](#セキュリティ境界)
- [可用性と冗長化](#可用性と冗長化)
- [コスト管理](#コスト管理)
- [運用SOP](#運用sop)
- [関連ドキュメント](#関連ドキュメント)

## 目的
Nyxプラットフォームのインフラ抽象仕様を定義し、環境設計・運用指針を明確化する。

## 環境構成
| 環境 | 目的 | 特徴 |
|------|------|------|
| Dev | 開発者検証 | 自動リソース立ち上げ、最小構成 |
| Staging | 総合テスト | 本番相当、スケール0.5 |
| Production | 本番 | 高可用性、地理分散 |
- 環境間の設定差分はConfigレジストリ (etcd) で管理。

## インフラコンポーネント
| 層 | サービス | 説明 |
|----|----------|------|
| コンピュート | コンテナオーケストレーション (例: Kubernetes互換) | ストリーム/ミックスノードを運用 |
| ストレージ | CockroachDB, オブジェクトストア | 状態管理、監査ログ |
| メッセージング | NATS JetStream | コントロールイベント、テレメトリ |
| 監視 | OpenTelemetry Collector, Grafana | メトリクス/トレース |
| セキュリティ | Vault互換、WAF | 鍵・シークレット管理、境界防御 |

## セキュリティ境界
- **ネットワーク分離**: Public, Control, Dataプレーンに分割。
- **Zero Trust**: すべてのサービス間でmTLS。
- **WAF/IDS**: API境界で異常検知。
- **秘密管理**: シークレットはKMS/Vaultで管理、ローテーション。Cライブラリ不要のクライアントを使用。

## 可用性と冗長化
- AZ3構成、リージョン2以上。
- コントロールプレーンは5ノードクォーラム。
- データプレーンはノード自動復旧。
- Disaster Recovery: 目標 RTO 15分, RPO 5分。

## コスト管理
- リソースタグでプロジェクト/環境を明示。
- Auto Scalingで非ピーク帯を削減。
- コスト予測は月次レビュー。

## 運用SOP
1. デプロイ計画: [deployment/ci-cd.md](./ci-cd.md)を参照。
2. 変更申請: CABで承認、ADR記録。
3. オンコール: 24/7体制、エスカレーション手順。
4. バックアップ: 日次スナップショット、週次整合性検証。
5. DRテスト: 半期に1度フルDR演習。

## 関連ドキュメント
- [deployment/ci-cd.md](./ci-cd.md)
- [deployment/rollback.md](./rollback.md)
- [security/vulnerability.md](../security/vulnerability.md)
- [performance/scalability.md](../performance/scalability.md)

> **宣言**: 実装コード無し、C/C++依存無し。