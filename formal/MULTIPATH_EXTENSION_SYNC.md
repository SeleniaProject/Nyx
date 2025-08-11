# Multipath Extension Formal Sync (@spec draft)

本ドキュメントは TLA+ `nyx_multipath_plugin.tla` / `nyx_advanced_features.tla` と実装 (nyx-stream multipath) の整合性検証差分メモです。

## 対象仕様ポイント
- Path activation/deactivation イベント
- 重み計算: RTT / jitter / loss / bandwidth (実装: recompute_weight)
- フェイルオーバ: is_healthy() 境界 (loss<0.5, RTT<5s, weight>0)
- 再順序復元: per-path + global buffer (config.reorder_global)
- Hop count 動的調整: loss>0.1 || rtt>500ms -> +1, 良好条件 -> -1

## 既存テスト対応
| Spec Concept | Test | 備考 |
|--------------|------|------|
| 同時利用 WRR 分配 | multipath_integration_distribution.rs | RTT から weight 推定し許容誤差で比率検証 |
| グローバル再順序 | multipath_end_to_end_reassembly.rs | per-path in-order + delay ソートシミュレーション |
| フェイルオーバ | NEW multipath_failover_end_to_end.rs | 劣化 RTT で除外→復旧 |
| 適応再順序/ジッタ Telemetry | multipath_unhealthy_path_bias.rs 等 | 既存メトリクス参照 |

## 未カバー / 今後
- 連続的損失注入による weight 減衰曲線 (プロパティベース)
- Hop count 遷移不変量 (MIN_HOPS..=MAX_HOPS) を TLA+ に昇格
- ReorderingBuffer グローバル+per-path 同時計測レース条件 (モデル検査)

## 次アクション
1. TLA+ モデルへ weight 動的範囲 (1..50_000) 制約追加
2. Hop count fairness (高 RTT パスが極端に低 weight 化しても最低選択頻度保持) を fairness property として追加
3. CI に model checking (最小状態空間) smoke を組込み
