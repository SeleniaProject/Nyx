# Weighted Round Robin Scheduler v2 解説

v2 スケジューラは Multipath 経路選択の分散と遅延最適化を同時に満たすため **Inverse-RTT 重み** と **平滑 Weighted Round Robin (SWRR)** を組み合わせています。

## 目標
1. 低 RTT 経路を優先しつつ不健全経路を完全孤立させない
2. 選択割合が理論重み比に近接 (許容誤差 ±~5–10%)
3. RTT/Loss 更新時の揺れ最小化 (Reweight shock 吸収)

## 重み計算
```
base = weight_scale / max(rtt_ms, 1)
loss_penalty = (1.0 - loss_rate).clamp(0.05, 1.0)
raw = base * loss_penalty
weight = clamp(raw.round(), 1, WEIGHT_CAP)
```
- `weight_scale` デフォ 1000 (10ms → 100, 20ms → 50)
- `WEIGHT_CAP` 極端な差を防ぐ上限 (例 10_000)
- Loss 反映で RTT だけでは説明できない品質差も低減

## SWRR アルゴリズム
各パス `i`:
```
current_i += weight_i
pick path with max(current_i)
current_i -= total_weight
```
- O(N) で簡潔・決定的 (random 不要) → テスト容易
- current 重みは `reset_weights()` で再初期化可能 (再接続イベント等)

### 選択割合の安定性
理論比: `weight_i / Σ weight`
テスト (`tests/multipath_integration_distribution.rs`) では 1600 選択で次を確認:
| 期待 | 実測範囲(assert) |
|------|------------------|
| 62.5% | 58–67% |
| 31.25% | 28–35% |
| 6.25% | 4–9% |

許容幅= ±~7% を初期指標。高分散ケースは ITER 増で収束。

## Loss / RTT 更新とデバウンス
- 連続急減 (RTT 大幅改善) 時は "weight jump" による burst 発生
- 対策: `update_path_rtt` 内部で *min-change threshold* (<5% なら無視) や *cooldown* を実装可能 (将来拡張)。現行はシンプル更新でテスト安定を優先。

## 不健全経路バイアス
`tests/multipath_unhealthy_path_bias.rs` にて高 RTT + 高 Loss 経路の選択割合 <10% を確認し、最低限の探索 (探索的パケット) 維持。

## Hop Count 動的調整との関係
Hop 数は PathStats が RTT / Loss を入力に別途調整: 遅延・損失が悪化 → Hop 数増 (匿名性向上) / 改善 → Hop 数減。WRR は hop 改変後も独立性を保つ。

## テレメトリ推奨
| メトリクス | 種別 | 説明 |
|------------|------|------|
| nyx_multipath_path_rtt_seconds | Histogram | RTT 分布 (後段重みチューニング) |
| nyx_multipath_path_activated_total | Counter | 新規パス活性数 |
| nyx_multipath_path_deactivated_total | Counter | パス終了数 |
| nyx_multipath_active_paths | Gauge | 現在の有効パス数 |

追加提案:
- Gauge: `nyx_wrr_weight_ratio_deviation` = 実測割合と理論比の平均偏差
- Histogram: `nyx_multipath_reorder_delay_seconds` = 再順序遅延分布
- GaugeVec: `nyx_multipath_reorder_buffer_utilization` = 再順序バッファ利用率

## 調整シナリオ
| 症状 | 原因 | チューニング |
|------|------|--------------|
| 高 RTT 経路完全停止 | weight_scale 過大 / loss_penalty 下限 | weight_scale 低下 / 下限値上げ |
| 低 RTT 経路独占 | WEIGHT_CAP 高すぎ | CAP 降下 (例 5000→2000) |
| 分布ゆらぎ (>10%) | 選択回数少 | 観測窓を長く / スロット数拡大 |

## 今後の拡張
- EWMA RTT (短期スパイク平滑化) 導入
- Relative Entropy ベースのフェアネス指標報告
- パス集合再最適化 (定期 prune + add strategy)

---
最終更新: 手動メンテ。テストで重み分布は自動検証済み。
