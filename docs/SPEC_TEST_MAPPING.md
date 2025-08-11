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

自動生成テーブル (セクションカバレッジ 100.0%: 10/10):

| Spec 節 | テストケース |
|---------|--------------|
| 1. Protocol Combinator (Plugin Framework) | nyx-stream/tests/plugin_framework_tests.rs::test_plugin_frame_type_validation<br>nyx-stream/tests/plugin_framework_tests.rs::test_plugin_header_cbor_encoding<br>nyx-stream/tests/plugin_framework_tests.rs::test_plugin_frame_building_and_parsing<br>nyx-stream/tests/plugin_framework_tests.rs::test_plugin_frame_size_limits |
| 10. Compliance Levels | nyx-conformance/tests/core.rs::nyx_config_parse_defaults |
| 2. Multipath Data Plane | nyx-stream/tests/multipath_failover_end_to_end.rs::multipath_failover_and_rejoin<br>nyx-stream/tests/multipath_integration_distribution.rs::multipath_wrr_distribution_matches_weights<br>nyx-stream/tests/weighted_round_robin_scheduler_v2.rs::test_scheduler_creation_and_basic_operations<br>nyx-stream/tests/weighted_round_robin_scheduler_v2.rs::test_path_management<br>nyx-stream/tests/weighted_round_robin_scheduler_v2.rs::test_weight_calculation_from_rtt<br>nyx-stream/tests/weighted_round_robin_scheduler_v2.rs::test_smooth_wrr_distribution<br>nyx-stream/tests/weighted_round_robin_scheduler_v2.rs::test_path_activation_deactivation |
| 3. Hybrid Post-Quantum Handshake | nyx-crypto/tests/kyber.rs::kyber_kem_session_key_matches<br>nyx-stream/src/tests/hpke_rekey_integration_tests.rs::hpke_rekey_triggers_on_packet_threshold<br>nyx-crypto/src/noise.rs::test_hybrid_message_too_short |
| 4. cMix Integration | nyx-conformance/tests/cmix.rs::cmix_batch_verification<br>nyx-conformance/tests/cmix_negative.rs::cmix_verify_rejects_tampered_batch<br>nyx-conformance/tests/cmix_negative.rs::cmix_verify_rejects_invalid_witness<br>nyx-conformance/tests/e2e_full.rs::e2e_full_stack<br>nyx-mix/src/cmix.rs::emits_batch_after_timeout<br>nyx-mix/src/cmix.rs::detailed_verification_reports_errors |
| 5. Adaptive Cover Traffic | nyx-mix/tests/adaptive_cover_feedback.rs::adaptive_cover_utilization_feedback_non_decreasing_lambda<br>nyx-stream/tests/multipath_integration_distribution.rs::multipath_wrr_distribution_matches_weights |
| 6. Low Power Mode (Mobile) | tests/integration/comprehensive_test_suite.rs::test_low_power_mode<br>tests/integration/production_integration_tests.rs::test_low_power_scenarios<br>nyx-mix/tests/low_power_screen_off_ratio.rs::low_power_screen_off_cover_ratio_applied |
| 6. Low Power Mode (Mobile) (redundant placeholder) | nyx-core/tests/low_power_screen_off_ratio.rs::low_power_screen_off_cover_ratio_applied |
| 7. Extended Packet Format | nyx-stream/tests/builder.rs::header_roundtrip<br>nyx-stream/tests/frame.rs::parse_basic_header<br>nyx-stream/tests/multipath_header_flags.rs::build_ext_sets_flags_and_appends_path_id |
| 7. FEC (Adaptive RaptorQ) | nyx-fec/tests/adaptive_raptorq_redundancy.rs::adaptive_raptorq_redundancy_adjusts_both_directions |
| 8. Capability Negotiation | nyx-conformance/tests/capability_negotiation_properties.rs::capability_id_strategy |
| 8. Capability Negotiation (frame build & parse) | nyx-stream/tests/plugin_framework_tests.rs::test_plugin_frame_building_and_parsing |
| 8. Capability Negotiation (handshake frame types) | nyx-stream/tests/plugin_framework_tests.rs::test_plugin_frame_type_validation |
| 8. Capability Negotiation (header encoding schema) | nyx-stream/tests/plugin_framework_tests.rs::test_plugin_header_cbor_encoding |
| 8. Capability Negotiation (size limits / enforcement) | nyx-stream/tests/plugin_framework_tests.rs::test_plugin_frame_size_limits |
| 8. System Metrics (Disk/FD/Thread) | nyx-daemon/tests/disk_fd_thread_metrics.rs::system_metrics_basic_smoke |
| 9. Telemetry Schema (OTLP) | nyx-stream/src/tests/hpke_rekey_integration_tests.rs::hpke_rekey_triggers_on_packet_threshold |

未マッピング節: なし

---
このセクション以下は自動生成されます。手動編集は次回上書きされます。
