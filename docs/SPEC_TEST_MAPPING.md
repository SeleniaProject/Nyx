# Spec-to-Test Mapping (初期スケルトン)

現状のテストには明示的な `@spec` アノテーションが存在しないため、自動抽出は未実装。本ファイルは将来の自動生成ターゲット仕様を定義するスケルトンです。

## 目的
- 仕様セクション (例: "2. Multipath Data Plane") と Rust テストケースを可逆にリンク
- 仕様変更時に影響テスト集合を即時求める
- カバレッジギャップ (未テスト仕様節) を可視化

## 想定アノテーション形式 (提案)
Rust テスト関数直上コメント:
```rust
/// @spec 2. Multipath Data Plane
/// @spec 5. Adaptive Cover Traffic
#[test]
fn multipath_distribution_converges() { /* ... */ }
```
複数節を列挙可。`@spec` トークン後に仕様節タイトル先頭の番号+タイトルをそのまま記載。

## 自動生成ツール (予定 `scripts/spec_test_map.py`)
1. `spec/Nyx_Protocol_v1.0_Spec_EN.md` から `## ` セクション抽出
2. `**/tests/**/*.rs` を走査し `@spec` 行を正規表現取得
3. JSON 出力例:
```json
{
  "sections": {
    "2. Multipath Data Plane": ["nyx-stream/tests/multipath_integration_distribution.rs::multipath_distribution_converges"],
    "5. Adaptive Cover Traffic": ["..."]
  },
  "unmapped_sections": ["6. Low Power Mode (Mobile)"]
}
```
4. Markdown 生成 (本ファイルを上書き)

## カバレッジ指標
- Section Coverage = (#mapped sections) / (総セクション数)
- Test Redundancy (任意) = セクションあたり平均テスト数

## 当面の手動ダイジェスト (抜粋)

自動生成テーブル (セクションカバレッジ 90.0%: 9/10):

| Spec 節 | テストケース |
|---------|--------------|
| 1. Protocol Combinator (Plugin Framework) | nyx-stream/tests/plugin_framework_tests.rs::test_plugin_frame_type_validation<br>nyx-stream/tests/plugin_framework_tests.rs::test_plugin_header_cbor_encoding<br>nyx-stream/tests/plugin_framework_tests.rs::test_plugin_frame_building_and_parsing<br>nyx-stream/tests/plugin_framework_tests.rs::test_plugin_frame_size_limits |
| 10. Compliance Levels | nyx-conformance/tests/core.rs::nyx_config_parse_defaults |
| 2. Multipath Data Plane | nyx-stream/tests/multipath_integration_distribution.rs::multipath_wrr_distribution_matches_weights<br>nyx-stream/tests/weighted_round_robin_scheduler_v2.rs::test_scheduler_creation_and_basic_operations<br>nyx-stream/tests/weighted_round_robin_scheduler_v2.rs::test_path_management<br>nyx-stream/tests/weighted_round_robin_scheduler_v2.rs::test_weight_calculation_from_rtt<br>nyx-stream/tests/weighted_round_robin_scheduler_v2.rs::test_smooth_wrr_distribution<br>nyx-stream/tests/weighted_round_robin_scheduler_v2.rs::test_path_activation_deactivation |
| 3. Hybrid Post-Quantum Handshake | nyx-crypto/tests/kyber.rs::kyber_kem_session_key_matches<br>nyx-stream/src/tests/hpke_rekey_integration_tests.rs::hpke_rekey_triggers_on_packet_threshold |
| 5. Adaptive Cover Traffic | nyx-stream/tests/multipath_integration_distribution.rs::multipath_wrr_distribution_matches_weights |
| 6. Low Power Mode (Mobile) | tests/integration/comprehensive_test_suite.rs::test_low_power_mode<br>tests/integration/production_integration_tests.rs::test_low_power_scenarios |
| 7. Extended Packet Format | nyx-stream/tests/builder.rs::header_roundtrip<br>nyx-stream/tests/frame.rs::parse_basic_header<br>nyx-stream/tests/multipath_header_flags.rs::build_ext_sets_flags_and_appends_path_id |
| 8. Capability Negotiation | nyx-conformance/tests/capability_negotiation_properties.rs::capability_id_strategy |
| 9. Telemetry Schema (OTLP) | nyx-stream/src/tests/hpke_rekey_integration_tests.rs::hpke_rekey_triggers_on_packet_threshold |

未マッピング節: 4. cMix Integration

---
このセクション以下は自動生成されます。手動編集は次回上書きされます。
