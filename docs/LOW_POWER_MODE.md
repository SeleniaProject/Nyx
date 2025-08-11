# 低電力モード (Low Power Mode)

Nyx の低電力モードはモバイル / バッテリ制約デバイス向けにカバートラフィックと周期タスクを動的抑制し、匿名性を過度に損なわずに消費電力を削減します。

## 目的
- 画面消灯・長時間バックグラウンドでの CPU / 無線利用削減
- カバートラフィック (ダミー送信) を適応的に引き下げ
- 再接続レイテンシと匿名性セット(Anonymity Set)のバランス維持

## トリガ
| イベント | 判定例 | アクション |
|----------|--------|-------------|
| 画面消灯 | OS API / Push 通知サスペンド | cover_ratio=0.1, keepalive=60s |
| バッテリ低下 | SoC < 15% | 再キー間隔延長 / 一部 Telemetry 間引き |
| 長時間アイドル | User I/O 無 5分 | ダミーフロー停止, 軽量 ping 継続 |

## パラメータ
| 名前 | 既定 | 説明 |
|------|------|------|
| cover_ratio_active | 0.4 | 通常時ダミー比率 (実データ:ダミー) |
| cover_ratio_low_power | 0.1 | 低電力時比率 |
| keepalive_interval_low | 60s | NAT / Path 維持用最小トラフィック |
| rtt_probe_suppression | true | 低電力中 RTT 詳細プローブ抑制 |

## 状態遷移 (簡略)
```
ACTIVE --(screen off / battery low)--> LOW_POWER
LOW_POWER --(user activity / charging)--> ACTIVE
```

## Grace Policy と HPKE
低電力遷移直後は突発的パケット欠損で再キー Grace 使用が増える可能性があります。`HPKE_REKEY_TELEMETRY.md` の `grace_used/applied` を監視し 0.2 を超える場合は cover_ratio_low_power を若干引き上げ (例 0.12 → 0.15)。

## 匿名性影響緩和
- 最低 Poisson λ を 10分当たり数パケットに維持
- シンクロナイズ抑制: 画面消灯イベントにランダムジッタ 0–5s を導入

## 推奨ダッシュボード指標
| メトリクス | 意味 | アラート指標 |
|------------|------|---------------|
| active_paths (Gauge) | Multipath 利用数 | 低電力遷移で急減 (<1) 継続 |
| hpke_rekey_grace_used_total / applied_total | Grace 比 | >0.2 (1h) |
| cover_traffic_rate (自前 Gauge) | カバートラフィック現在値 | 0 近傍長時間 |

## 今後の拡張
- 端末温度連動で更なるダミー削減
- ML によるユーザ行動予測で事前遷移

---
最終更新: 手動メンテ (spec_diff.py に含まれません)
