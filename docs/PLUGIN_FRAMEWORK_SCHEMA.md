# Plugin Framework Schema (v1.0)

本ドキュメントでは Nyx Stream 層のプラグインフレームワーク v1.0 における
メタデータ / スキーマ / Telemetry カウンタをまとめる。コード実装は `nyx-stream/src/plugin_registry.rs` などを参照。

## 1. PluginInfo メタデータ構造

| フィールド | 型 | 説明 |
|------------|----|------|
| id | u32 | 一意な PluginId |
| name | String | 表示名 |
| version | String (SemVer) | バージョン |
| description | String | 説明 |
| permissions | Vec<Permission> | 権限制御列挙 |
| author | String | 作者/ベンダ |
| config_schema | HashMap<String, String> | 設定キー → 簡易スキーマ表現 (型 / 制約の短い文字列) |
| supported_frames | Vec<u8> | サポートするフレーム種別 ID |
| required | bool | プロトコル必須か |

Permission 列挙は `ReceiveFrames / Handshake / DataAccess / Control / ErrorReporting / NetworkAccess / FileSystemAccess / InterPluginIpc / AccessGeo / ACCESS_NETWORK / ACCESS_GEO / PluginPersistence / CryptoAccess / MetricsAccess` を含む。

## 2. Config Schema 仕様

`config_schema` の値は軽量な**人可読**表現で、厳密 JSON Schema ではない。将来:

```
"interval_ms": "integer >= 10"
"enable_cache": "boolean"
"mode": "enum[fast,balanced,secure]"
```

など。テスト `register_and_fetch_roundtrip` が RoundTrip を保証。

## 3. Telemetry カウンタ (nyx-telemetry)

| メトリック | 意味 |
|-------------|------|
| nyx_plugin_init_success_total | 初期化成功 |
| nyx_plugin_init_failure_total | 初期化失敗 |
| nyx_plugin_security_pass_total | セキュリティ検証成功 |
| nyx_plugin_security_fail_total | セキュリティ検証失敗 |
| nyx_hpke_rekey_initiated_total | HPKE 再鍵決定 |
| nyx_hpke_rekey_applied_total | HPKE 新鍵適用 |
| nyx_hpke_rekey_grace_used_total | Grace Window 旧鍵使用 |
| nyx_hpke_rekey_fail_total | HPKE 再鍵失敗 |
| nyx_multipath_packets_sent_total | Multipath 送信 |
| nyx_multipath_path_activated_total | パス活性化 |
| nyx_multipath_path_deactivated_total | パス非活性 |

## 4. 今後の拡張予定

- JSON Schema v7 互換の自動生成 ( `schemars` 活用 )
- Permission 依存解決 (他プラグイン ID 宣言)
- 署名付き PluginInfo (Ed25519) による信頼連鎖

## 5. テストカバレッジ

`plugin_registry.rs` に以下テスト:
1. `register_and_fetch_roundtrip` : スキーマ含む登録/取得検証
2. `duplicate_registration_fails` : 重複登録エラー
3. `unregister_removes` : 正常削除検証

---
最終更新: 自動生成 (バックログ Section M 対応)。
