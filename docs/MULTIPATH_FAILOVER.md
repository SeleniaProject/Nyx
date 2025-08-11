# Multipath Failover / Failback & Telemetry

本ドキュメントは複数経路 (Multipath) 利用時のフェイルオーバ / フェイルバック動作と関連テレメトリ指標を定義し、運用・チューニング手順を示す。

## 目的
- 経路障害時の復旧時間 (MTTR) 最小化
- 過剰切替 (flapping) 抑制
- RTT / Loss / Jitter 変動下でのスループット安定化

## 状態マシン
```
INACTIVE -> PROBING -> ACTIVE -> DEGRADED -> (RECOVERY|REPLACE) -> ACTIVE | INACTIVE
```
| 状態 | 条件 | 説明 |
|------|------|------|
| INACTIVE | 未選択 | 監視対象外 (最小ヘルスチェックのみ) |
| PROBING | 新規/再試行 | 軽量プローブ (低頻度 ping) |
| ACTIVE | ヘルス基準達成 | WRR 対象; 通常トラフィック転送 |
| DEGRADED | RTT/Loss 閾値超過 | 重み低下 (WRR); 継続監視 |
| RECOVERY | 改善兆候 | RTT/Loss 部分回復、連続良好サンプル確保中 |
| REPLACE | 代替探索 | 新規パス候補が安定 → 旧パス縮退 |

## 遷移条件 (例値)
| 遷移 | 条件 (連続サンプル) |
|------|----------------------|
| PROBING→ACTIVE | success >=3 / last 4 |
| ACTIVE→DEGRADED | rtt_p95 > rtt_slo_ms OR loss_rate > 0.08 for 5 連続 |
| DEGRADED→RECOVERY | rtt_p95 < 0.9*rtt_slo AND loss_rate < 0.05 for 3 連続 |
| RECOVERY→ACTIVE | 条件持続 + 追加 2 サンプル |
| ANY→INACTIVE | hard_fail >=2 (timeout, auth fail 等) |
| DEGRADED→REPLACE | 代替候補 ACTIVE 維持 + 現パス rtt_p95 悪化 15%+ で 10 サンプル |

(閾値は `nyx.toml` で将来構成可能予定)

## ヘルス計測
- RTT EWMA (α=0.3) + p95 スライディングウィンドウ (サイズ 20)
- Loss rate: 直近 50 パケット
- Jitter: RTT 差分の標準偏差 (窓 20)

## WRR との連携
- DEGRADED パス: weight *= 0.4 (最小 1 維持)
- RECOVERY 中: 緩和 weight = 0.7 * 正常計算
- REPLACE トリガ: 旧パス weight 減衰 factor 0.5 を 2 ラウンド適用後 INACTIVE

## テレメトリ指標
| メトリクス | 種別 | ラベル | 説明 |
|------------|------|--------|------|
| nyx_multipath_path_rtt_seconds | Histogram | path_id | RTT 分布 |
| nyx_multipath_path_jitter_seconds | Histogram | path_id | Jitter 分布 (ΔRTT) |
| nyx_multipath_path_packet_loss_rate | Gauge | path_id | 直近窓 Loss |
| nyx_multipath_path_state | Gauge | path_id,state | 現在=1, 他=0 (状態エンコード) |
| nyx_multipath_failover_total | Counter | from_state,to_state | フェイルオーバ回数 |
| nyx_multipath_failback_total | Counter | - | フェイルバック (DEGRADED/RECOVERY→ACTIVE) |
| nyx_multipath_path_replaced_total | Counter | old_path_id,new_path_id | 置換完了 |
| nyx_wrr_weight_ratio_deviation | Gauge | - | 実測/期待 重み偏差 |
| nyx_multipath_reorder_delay_seconds | Histogram | - | 受信〜配信までの再順序遅延 |
| nyx_multipath_reorder_buffer_utilization | Gauge | path_id | 再順序バッファ利用率 (0-1) |

### OpenTelemetry Span (任意)
- name: MultipathFailover
  - attrs: path_id, prev_state, new_state, reason, rtt_p95, loss_rate
- name: MultipathRecovery
  - attrs: path_id, stable_samples, rtt_reduction_pp (改善率)

## アラート推奨
| アラート | 例条件 |
|----------|--------|
| 連続 failover | nyx_multipath_failover_total Δ > 5 /5m |
| 高 Loss 持続 | avg(loss_rate) > 0.1 for 10m |
| 高 Jitter | jitter_p95 > 2 * baseline for 5m |
| 重み偏差拡大 | nyx_wrr_weight_ratio_deviation > 0.15 for 15m |

## 運用プレイブック
| 症状 | 調査 | 対処 |
|------|------|------|
| failover 頻発 | RTT/ Loss トレンド, state 変遷 | 閾値緩和 or cooldown 延長 |
| 回復遅延 | RECOVERY 滞留時間 | recovery 連続サンプル要件削減 (3→2) |
| 高 Loss 継続 | パス品質, 代替候補有無 | 早期 REPLACE トリガ (悪化%閾値 15→10) |
| 重み偏差高 | 分布テレメトリ | weight_scale 見直し / RTT smoothing 強化 |

## 将来拡張
- 動的閾値 (全体平均 RTT 分布に基づく percentile 閾値計算)
- 信頼区間ベースの安定判定 (Wilson interval)
- パススコアリング (多変量: RTT, Loss, Jitter, Variance)

---
最終更新: 手動メンテ (WRR_SCHEDULER_V2.md と併読推奨)。
