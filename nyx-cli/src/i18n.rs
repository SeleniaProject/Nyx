//! Minimal i18n tables for CLI messages (EN/JA) with safe fallback.

#![forbid(unsafe_code)]

use std::collections::HashMap;

pub type I18nTable = HashMap<&'static str, &'static str>;

fn base_en() -> I18nTable {
	// Keep keys stable and short; values are static &'static str
	let pairs: [(&str, &str); 16] = [
		("app.title", "Nyx CLI"),
		("app.version", "Version"),
		("app.error", "Error"),
		("cmd.connect", "Connect to daemon"),
		("cmd.disconnect", "Disconnect"),
		("cmd.status", "Show status"),
		("cmd.config", "Manage configuration"),
		("cmd.config.reload", "Reload configuration"),
		("cmd.config.update", "Update configuration"),
		("cmd.events", "Subscribe to events"),
		("hint.token", "Provide control token with --token or NYX_TOKEN/NYX_CONTROL_TOKEN"),
		("msg.connected", "Connected"),
		("msg.disconnected", "Disconnected"),
		("msg.reloading", "Reloading configuration..."),
		("msg.updated", "Configuration updated"),
		("msg.subscribed", "Subscribed to events"),
	];
	pairs.into_iter().collect()
}

fn ja_overlay() -> I18nTable {
	// Overlay for Japanese; only keys that differ from EN are included
	let pairs: [(&str, &str); 14] = [
		("app.title", "Nyx CLI"),
		("app.version", "バージョン"),
		("app.error", "エラー"),
		("cmd.connect", "デーモンに接続"),
		("cmd.disconnect", "切断"),
		("cmd.status", "ステータス表示"),
		("cmd.config", "設定管理"),
		("cmd.config.reload", "設定を再読み込み"),
		("cmd.config.update", "設定を更新"),
		("cmd.events", "イベント購読"),
		("hint.token", "--token または NYX_TOKEN/NYX_CONTROL_TOKEN で制御トークンを指定してください"),
		("msg.connected", "接続しました"),
		("msg.disconnected", "切断しました"),
		("msg.reloading", "設定を再読み込み中..."),
	];
	pairs.into_iter().collect()
}

fn normalize_lang(lang: &str) -> &str {
	let l = lang.trim().to_ascii_lowercase();
	if l.starts_with("ja") || l.contains("jp") || l.contains("jpn") { "ja" } else { "en" }
}

/// Get i18n table for a language code; falls back to English and overlays
/// language-specific entries. Unknown or empty language => English.
pub fn get_table(lang: &str) -> I18nTable {
	let mut map = base_en();
	match normalize_lang(lang) {
		"ja" => {
			for (k, v) in ja_overlay() { map.insert(k, v); }
		}
		_ => {}
	}
	map
}

