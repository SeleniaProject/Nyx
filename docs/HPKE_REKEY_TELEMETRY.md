# HPKE リキー テレメトリ概要

本ドキュメントは HPKE セッションキー再キー (rekey) のライフサイクルと、`nyx-telemetry` が公開するメトリクスの意味・活用方法をまとめます。

## 対象読者
- 運用者: 再キー頻度と失敗を監視したい
- セキュリティ監査: Grace Window 利用回数を評価し前方秘匿性 (PFS) 上のリスクを判断したい
- パフォーマンスチューニング: 再キー適用遅延を把握したい

## ライフサイクル概要
1. 閾値到達 (パケット数 / 経過時間 / PCR イベント) により再キー決定 → `initiated`
2. 新鍵生成 & HPKE コンテキスト再初期化
3. 並行期間: 旧鍵は Grace Window (例: 30s or N パケット) 内で復号フォールバックに利用可能 (`grace_used` カウント)
4. 新鍵のみ運用 / 旧鍵破棄 (zeroize)

## Prometheus メトリクス
| 名前 | 種別 | 説明 | 運用上の指標例 |
|------|------|------|----------------|
| `nyx_hpke_rekey_initiated_total` | Counter | 再キー決定回数 | 期待値: ポリシー通り (過剰増加は閾値が低すぎ) |
| `nyx_hpke_rekey_applied_total` | Counter | 新鍵が適用された回数 | `initiated` との差分は失敗/遅延 |
| `nyx_hpke_rekey_grace_used_total` | Counter | Grace Window 中 旧鍵での復号回数 | 高すぎると送信側/ネットワーク遅延増大の兆候 |
| `nyx_hpke_rekey_fail_total` | Counter | 再キー失敗 (生成/復号失敗) | 非ゼロは即調査 |

### 典型的なアラート条件 (例)
- 5 分間で `nyx_hpke_rekey_fail_total` 増分 > 0 → Critical
- `nyx_hpke_rekey_initiated_total - nyx_hpke_rekey_applied_total > 3` → Warning (未適用滞留)
- `grace_used / applied > 0.2` (直近 1h) → Info: ネットワーク遅延 or RTT ばらつき増加

## Grace Window の解釈
`grace_used` は「旧鍵がまだ必要だった復号試行」の粗近似。大量発生は:
- パケット再送/遅延 (遅いパス) が多い
- Multipath スケジューラの遅延差が大きい
- 再キー頻度が高すぎ同期が追いつかない

## 運用チューニングの指針
| 症状 | 観測 | 改善アクション |
|------|------|----------------|
| 過剰な再キー | `initiated` 急増 | 閾値 (時間/パケット) 引き上げ |
| 適用遅延 | `initiated - applied` 拡大 | キー生成並列度 / I/O 負荷確認 |
| Grace 過剰 | `grace_used/applied` 高 | RTT 差分調査 / Multipath パス品質調整 |
| 失敗発生 | `fail_total` > 0 | ログ (reason) 解析 / ライブラリアップデート |

## ダッシュボード例 (Grafana)
- パネル 1: 再キー総数 (Initiated / Applied stacked)
- パネル 2: Fail Counter (5m rate)
- パネル 3: Grace 使用率 = increase(grace_used_total) / increase(applied_total)
- パネル 4: RTT ヒストグラム (Multipath) と突発遅延相関

## テストとの対応
| テストファイル | 検証内容 |
|-----------------|----------|
| `nyx-stream/src/tests/hpke_rekey_integration_tests.rs` | Initiated / Applied / Failure / Grace 使用カウンタ増分 |
| `nyx-stream/src/hpke_rekey_manager.rs` (unit test) | Grace decrypt パス直接行使 |

## 今後の拡張候補
- 再キー適用レイテンシ Histogram
- 旧鍵同時保持時間 Gauge
- 鍵素材ゼロ化遅延メトリクス

---
最終更新: 自動生成されていません (手動メンテ対象)。
