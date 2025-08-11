# Changelog

## [1.0.0] - 2025-08-11
### Added
- Plugin Frame (0x50–0x5F) JSON Schema 自動生成 (`nyx-stream/src/bin/generate_plugin_schema.rs`) と `PluginHeader` / `PluginFrame` `JsonSchema` 派生。
- Mobile Power / Push Notification 統合ガイド `docs/MOBILE_POWER_PUSH_INTEGRATION.md` 追加。
- Daemon `NodeInfo` に compile-time feature からの Capability 集約と Compliance Level (Core/Plus/Full) 判定ロジック統合 (spec §10)。
- Peer Authentication Guide に Low Power / Push 連携セクション追加。

### Changed
- `plugin_frame.rs` に JSON Schema エクスポートユーティリティ追加。
- `plugin.rs` に各 CBOR 構造へ `JsonSchema` 派生付与。

### Documentation
- `IMPLEMENTATION_REPORT.md` / `task.md` M セクション進捗同期 (v1.0 Draft-Complete 差分反映)。

### Notes
- Compliance 判定は `nyx_core::compliance::determine` を利用し将来 runtime capability 拡張に対応可能な設計。
