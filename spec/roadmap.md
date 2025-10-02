# docs/roadmap.md

> **ポリシー** : :no_entry: 実装コード非出力 / :no_entry_sign: C/C++依存禁止（[要件遵守](./requirements.md#遵守事項)）

## 目次
- [ロードマップの前提](#ロードマップの前提)
- [フェーズ構成とゴール](#フェーズ構成とゴール)
- [リリースポリシー](#リリースポリシー)
- [リスクと緩和策](#リスクと緩和策)
- [依存関係マップ](#依存関係マップ)
- [監視指標と意思決定ゲート](#監視指標と意思決定ゲート)

## ロードマップの前提
- **プロジェクトミッション**: [docs/README.md](./README.md#ビジョンと位置づけ)
- **要求管理**: [requirements.md](./requirements.md) による要件IDトレーサビリティ
- **アーキテクチャ基盤**: [architecture/overview.md](./architecture/overview.md)
- **性能/セキュリティ基準**: [performance/scalability.md](./performance/scalability.md), [security/vulnerability.md](./security/vulnerability.md)

## フェーズ構成とゴール
| フェーズ | 期間目安 | 主要成果物 | Entry基準 | Exit基準 | 連動ドキュメント |
|----------|----------|------------|------------|-----------|------------------|
| **Phase \u03b1: Foundation** | 0-3ヶ月 | プロトコル基礎仕様・初期ノード実装準備 | 要件ID確定 (REQ-100前後) | ハイブリッド握手仕様の承認 | [architecture/interfaces.md](./architecture/interfaces.md), [security/encryption.md](./security/encryption.md)
| **Phase \u03b2: Core Network** | 3-6ヶ月 | 安全ストリーム層とMixルーティングPoC | Phase \u03b1 Exit | データフローテスト完了 (SEQUENCE-01) | [architecture/dataflow.md](./architecture/dataflow.md), [testing/integration-tests.md](./testing/integration-tests.md)
| **Phase \u03b3: Adaptive Ops** | 6-9ヶ月 | カバートラフィック適応、動的経路最適化 | Phase \u03b2 Exit + 性能ベンチ初回完了 | P95遅延目標達成 (350ms) | [performance/benchmark.md](./performance/benchmark.md)
| **Phase \u03b4: Ecosystem** | 9-12ヶ月 | SDK/UX整備、監査ログ、CI/CD | Phase \u03b3 Exit + セキュリティレビュー | RBAC/ABAC設計完遂 + UI規範承認 | [ui/overview.md](./ui/overview.md), [security/auth.md](./security/auth.md), [deployment/ci-cd.md](./deployment/ci-cd.md)
| **Phase \u03b5: Harden & Launch** | 12-15ヶ月 | 本番環境準備、SLA合意、トラステッドローンチ | Phase \u03b4 Exit + 全テスト緑 | 上場・主要顧客トライアル完了 | [deployment/infrastructure.md](./deployment/infrastructure.md), [deployment/rollback.md](./deployment/rollback.md)

## リリースポリシー
- **Release Cadence**: 6週間スプリントのダブルダイヤモンド構造。奇数スプリントで探索・偶数スプリントで収束。
- **ブランチ戦略**: `main`は常時リリース可能状態、`release/<version>`で候補を扱い、仕様変更は`docs/`配下から着手。
- **バージョニング**: セマンティックバージョニングに準拠。仕様変更はマイナー/メジャーで明示。
- **リリース判定ゲート**: KPI/SLIが[testing/metrics.md](./testing/metrics.md)の"リリースゲート"セクションを満たすこと。

## リスクと緩和策
| リスク | 影響 | 兆候 | 緩和策 | 監視 | 撤退指標 |
|--------|------|------|--------|------|-----------|
| PQ暗号成熟度不足 | セキュリティ低下 | NIST最終標準遅延 | 二重鍵交換の維持とサンドボックス評価 | [security/encryption.md](./security/encryption.md) | PQ実装に重大脆弱性が発覚 |
| モバイル性能劣化 | UX低下 | バッテリー消費急増 | FEC率とカバー交通をアダプティブ制御 | [performance/scalability.md](./performance/scalability.md) | モバイル離脱率 > 10% |
| 規制変更 | 導入阻害 | 新規法案の審議 | 法務チームと月次レビューボード設置 | [security/vulnerability.md#監査と法的遵守](./security/vulnerability.md#監査と法的遵守) | 特定市場で導入禁止 |
| ノード乗っ取り | 信頼低下 | 異常トラフィック検知 | ゼロトラスト監査ログと自動隔離 | [security/auth.md#監査](./security/auth.md#監査) | 重大インシデント > 1/月 |
| UX複雑化 | 採用阻害 | サポート問い合わせ増 | UIガイドラインのUX監査 | [ui/accessibility.md](./ui/accessibility.md) | オンボード完了率 < 80% |

## 依存関係マップ
```
Phase α --> Phase β --> Phase γ --> Phase δ --> Phase ε
   |           |           |           |
   v           v           v           v
[requirements] [architecture/overview] [performance/scalability] [deployment/infrastructure]
```
- 技術依存: 暗号スイート選定がPhase βの手戻りを防ぐ。
- 組織依存: ガバナンス委員会設置（Phase α Exit条件）。
- 外部依存: PQ暗号標準化の進度、学術連携の評価結果。

## 監視指標と意思決定ゲート
- **Go/No-Go判定ポイント**: 各フェーズExit時に、SLI/SLO達成、セキュリティ残留リスク、UXスコア、運用成熟度を総合判断。
- **メトリクスダッシュボード**: [testing/metrics.md](./testing/metrics.md)で定義した指標をGrafana等に可視化。
- **ADR連携**: 重大な方向転換は[notes/decision-log.md](./notes/decision-log.md)に記録。