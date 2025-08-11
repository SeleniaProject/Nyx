# Mobile Power Mode & Push Notification Integration Guide

本ガイドは NyxNet モバイル環境 (Android / iOS) において Low Power Mode と Push Notification Path を統合し、最小バッテリ消費で遅延を抑えつつ経路可用性を確保する実装 / 運用手順を示します。

## 1. 概要
- Low Power Mode: 画面オフ / バッテリ残量 / アプリバックグラウンド状態をシグナルとしてカバートラフィックレート・再鍵間隔・プロービング頻度を動的縮小。
- Push Notification Path: モバイル OS の push (FCM/APNs) 経由で「再接続トリガ」や "wake" シグナルを受け、Gateway 経路を迅速再確立。
- 目的: 平常時匿名性維持 / 休止時バッテリ節約 / 復帰時レイテンシ最小化。

## 2. Power State モデル
| State | トリガ条件 (例) | cover_ratio 推奨 | プロービング | 再鍵間隔 | 備考 |
|-------|----------------|-----------------|--------------|---------|------|
| Active | 画面オン / ユーザ操作直後 | 1.0 | 標準 (RTT+帯域) | 通常 | フル匿名性 |
| Background | アプリ BG / 画面オフ直後 | 0.4 | 減衰 (RTTのみ低頻度) | +25% | 軽負荷維持 |
| Inactive | 10–30m 無操作 | 0.1 | 停止 (passive) | +50% | Push 起点復帰 |
| Critical | 電池 <15% | 0.05 | 停止 | +100% | 緊急節約 |

内部 `power.state.changed` イベントを daemon が発火し、nyx-stream の adaptive cover controller と HPKE rekey scheduler に通知。

## 3. Push Notification Path
1. モバイルアプリは起動時に `device_push_token` を取得し daemon FFI API へ登録。
2. Gateway ノードは対象 peer の Quiet 状態 (Inactive/Critical) を検知でキューイング。
3. イベント (新メッセージ / 再鍵必要 / 重要経路更新) 発生時: push プロバイダ(Firebase/APNs) 経由で wake 信号送達。
4. アプリは起床後 3 秒以内に `NyxDaemon::resume_low_power_session()` を呼び path_builder が最小セット (1 control + 1 data) を再確立。

セキュリティ: Push payload は最小 (トピック + nonce) でアプリ内暗号キーにより AEAD 包装、復号失敗は無視。

## 4. FFI 連携 (概要 API)
```rust
// Power state 更新 (OS イベント受領時)
fn nyx_power_set_state(state: NyxPowerState);
// Push wake トリガ
fn nyx_push_wake();
// 再接続速攻確立 (small path set)
fn nyx_resume_low_power_session();
```

## 5. Peer Authentication との連携
- 初回ペアリング時に push 対応 capability (LOW_POWER / PUSH_GATEWAY) を advertisement。
- 認証後 trust スコアが閾値以上でのみ push wake を許可。
- 詳細: `PEER_AUTHENTICATION_GUIDE.md` セクション「Low Power / Push Interop」参照。

## 6. Telemetry / メトリクス
| Metric | 説明 |
|--------|------|
| power_state_transitions_total | 状態遷移総数 |
| push_wake_events_total | 受信 push wake 数 |
| low_power_reconnect_latency_ms | wake から最初の path ready まで p50/p95 |
| suppressed_cover_packets_total | Low Power で抑制された cover 数 |

## 7. 推奨テスト
- 画面オン→オフ→オン 循環で cover_ratio 適応を検証。
- Inactive 中 push wake 後 ~3s 以内にメッセージ受信。
- Critical バッテリで再鍵間隔延伸がメトリクス反映。

## 8. 今後の拡張
- OS ネイティブ省電力 API (Doze / BackgroundTasks) の統合。
- ML ベース使用パターン予測による事前 wake。
- Gateway 動的選択最適化 (地理+信頼+RTT)。

---
更新日: 2025-08-11
