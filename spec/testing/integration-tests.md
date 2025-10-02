# docs/testing/integration-tests.md

> **遵守バッジ** : :no_entry: 実装コード非出力 / :no_entry_sign: C/C++依存禁止

## 目次
- [目的](#目的)
- [テストスコープ](#テストスコープ)
- [契約テスト方針](#契約テスト方針)
- [環境依存の抽象化](#環境依存の抽象化)
- [データ管理](#データ管理)
- [実行フロー](#実行フロー)
- [合否ゲート](#合否ゲート)
- [関連ドキュメント](#関連ドキュメント)

## 目的
Nyxシステムの統合テスト戦略を定義し、コンポーネント間契約の整合性を検証する。

## テストスコープ
| テストID | 範囲 | 対象 |
|----------|------|------|
| INT-01 | クライアント \u2192 Stream \u2192 Mix | セッション確立 |
| INT-02 | Mix \u2192 Obfuscation \u2192 Transport | データ転送 |
| INT-03 | Control Plane \u2192 全層 | ポリシー配布 |
| INT-04 | Observability | メトリクス収集 |
| INT-05 | Audit | 監査ログフロー |

## 契約テスト方針
- I/F仕様は[architecture/interfaces.md](../architecture/interfaces.md)の契約に従う。
- コンシューマ駆動契約 (CDC) を採用。契約変更はADRで承認。
- 各契約はシリアライゼーション、検証、エラーコードを検査。

## 環境依存の抽象化
- 本番依存サービスはモックサーバ/エミュレータ (Go/Python純実装)。
- KMS、Directory、Telemetryストアはスタブを用意。
- ネットワークラボ: 仮想ネットワークで遅延/損失を注入。

## データ管理
- テストデータは匿名化し、`testdata/`にバージョン管理。
- 実行ごとにRESET手順を実施。
- データ生成は[templates/test-template.md](../templates/test-template.md)の形式。

## 実行フロー
1. テスト環境起動。
2. 契約テスト (CDC) 実行。
3. 統合シナリオ再生 (INT-01〜05)。
4. 結果を記録、リグレッションレポート生成。

## 合否ゲート
- Critical/Highの欠陥はリリースブロック。
- 契約破壊 (互換性違反) は即時修正。
- 成果は[testing/metrics.md](./metrics.md)へ集約。

## 関連ドキュメント
- [testing/unit-tests.md](./unit-tests.md)
- [testing/e2e-tests.md](./e2e-tests.md)
- [architecture/interfaces.md](../architecture/interfaces.md)

> **宣言**: 実装コード無し、C/C++依存無し。