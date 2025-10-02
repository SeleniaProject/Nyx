# docs/deployment/ci-cd.md

> **遵守バッジ** : :no_entry: 実装コード非出力 / :no_entry_sign: C/C++依存禁止

## 目次
- [目的](#目的)
- [パイプライン構造](#パイプライン構造)
- [品質ゲート](#品質ゲート)
- [リリース戦略](#リリース戦略)
- [サプライチェーンセキュリティ](#サプライチェーンセキュリティ)
- [ロールアウト/ロールバック基準](#ロールアウトロールバック基準)
- [監視と通知](#監視と通知)
- [関連ドキュメント](#関連ドキュメント)

## 目的
NyxプロジェクトのCI/CDパイプラインを抽象化し、ビルド・検証・デプロイのベストプラクティスを定義する。

## パイプライン構造
1. **Source Stage**: コミット検証、SBOM生成。
2. **Build Stage**: コンポーネントビルド、テストフィクスチャ生成。
3. **Test Stage**: 単体→統合→E2E順で実行。
4. **Security Stage**: SAST/DAST、依存脆弱性チェック。
5. **Deploy Stage**: Canary → Progressive → GA。
- パイプラインは宣言的に管理し、C/C++依存のビルドツールは禁止。

## 品質ゲート
| ステージ | ゲート条件 | 失敗時対応 |
|----------|------------|------------|
| Build | ビルド成功、SBOM生成 | 修正まで停止 |
| Test | テスト緑、カバレッジ目標達成 | フィードバックループ |
| Security | 脆弱性 Critical=0 | セキュリティレビュー |
| Deploy | SLO順守、監査ログ整合 | リリース延期 |

## リリース戦略
- **カナリア**: 5%ノードから段階的拡大。
- **ブルー/グリーン**: 主要リリース時に利用。
- **Feature Flag**: 匿名性機能などを段階的に公開。
- リリース可否は[testing/metrics.md](../testing/metrics.md)の指標。

## サプライチェーンセキュリティ
- 署名されたアーティファクト、SLSAレベル2を目標。
- 依存チェック: ソフトウェア構成分析 (SCA)。
- 署名はシークレット管理 (Vault) を利用。

## ロールアウト/ロールバック基準
- ロールアウト: Canary SLO達成、監査ログ差分なし。
- ロールバック: SLO逸脱、匿名性指数低下、セキュリティインシデント。
- 手順は[deployment/rollback.md](./rollback.md)を参照。

## 監視と通知
- パイプラインメトリクス: 成功率、リードタイム。
- 通知: Slack/Teams等、重大障害はオンコール。
- 監査ログ: 変更履歴を記録し、RBAC適用。

## 関連ドキュメント
- [deployment/infrastructure.md](./infrastructure.md)
- [deployment/rollback.md](./rollback.md)
- [testing/metrics.md](../testing/metrics.md)
- [security/vulnerability.md](../security/vulnerability.md)

> **宣言**: 実装コード無し、C/C++依存無し。