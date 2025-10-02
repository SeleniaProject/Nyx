# docs/testing/metrics.md

> **遵守バッジ** : :no_entry: 実装コード非出力 / :no_entry_sign: C/C++依存禁止

## 目次
- [目的](#目的)
- [メトリクス体系](#メトリクス体系)
- [KPI/SLI/SLO一覧](#kpislislo一覧)
- [欠陥管理指標](#欠陥管理指標)
- [MTTR/MTBF](#mttrmtbf)
- [ゲート設計](#ゲート設計)
- [ダッシュボード設計](#ダッシュボード設計)
- [関連ドキュメント](#関連ドキュメント)

## 目的
品質指標を統合管理し、テスト・運用・リリース判断の共通基準を提供する。

## メトリクス体系
```
KPI (ビジネス) ──┐
                   ├→ SLO (ユーザ視点)
SLI (技術メトリクス) ┘
```
- KPI: 戦略指標
- SLO: ユーザ体験
- SLI: 観測値

## KPI/SLI/SLO一覧
| 名称 | 種別 | 定義 | 目標値 | 測定頻度 |
|------|------|------|--------|----------|
| Anonymous Session Success | KPI | 匿名セッション成功率 | 99.5% | 日次 |
| P95 Latency | SLI | セッション遅延 | 350ms以下 | 1分 |
| Cover Traffic Compliance | SLI | λ適合率 | 90%以上 | 5分 |
| Audit Log Freshness | SLI | 生成遅延 | 1秒以下 | リアルタイム |
| Uptime | SLO | サービス稼働率 | 99.95% | 月次 |
| PQ Negotiation Success | SLI | PQ握手成功率 | 98%以上 | 日次 |

## 欠陥管理指標
| 指標 | 定義 | 目標 |
|------|------|------|
| 欠陥密度 | リリース当たり重大欠陥数 / 機能数 | < 0.2 |
| エスケープ率 | 本番発見 / 総欠陥 | < 5% |
| バグ修正リードタイム | 重大欠陥の平均修正時間 | < 48時間 |

## MTTR/MTBF
- **MTTR**: インシデント検知から復旧まで < 15分。
- **MTBF**: 重大インシデント間隔 > 60日。
- 測定はIncident管理ツールから自動収集。

## ゲート設計
| ゲート | 条件 |
|--------|------|
| Merge Gate | 単体/統合テスト成功、SLI劣化なし |
| Release Gate | SLO達成、重要欠陥ゼロ、監査ログ整合 |
| Hotfix Gate | 根本原因分析と回帰テスト完了 |

## ダッシュボード設計
- カード: KPI表示。
- チャート: ラインチャート (遅延)、バー (欠陥)。
- テーブル: インシデントリスト。
- アラート: SLA破り、カバー交通異常。

## 関連ドキュメント
- [testing/unit-tests.md](./unit-tests.md)
- [testing/integration-tests.md](./integration-tests.md)
- [testing/e2e-tests.md](./e2e-tests.md)
- [performance/scalability.md](../performance/scalability.md)
- [deployment/ci-cd.md](../deployment/ci-cd.md)

> **宣言**: 実装コード無し、C/C++依存無し。