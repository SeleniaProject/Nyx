#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Lightweight i18n loader for .ftl-like key=value files.
/// This avoids bringing heavy dependencies while providing basic localization.
pub struct I18nCatalog {
    entries: HashMap<String, String>,
}

impl I18nCatalog {
    /// Load catalog for a language code from `nyx-cli/i18n/{lang}.ftl`.
    pub fn load(lang: &str) -> Self {
        // Allow only known languages; fallback to English otherwise
        let normalized = match lang {
            "ja" | "en" | "zh" => lang,
            _ => "en",
        };
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("i18n")
            .join(format!("{}.ftl", normalized));
        let mut entries = HashMap::new();
        if let Ok(text) = fs::read_to_string(&path) {
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') { continue; }
                if let Some((k, v)) = trimmed.split_once('=') {
                    entries.insert(k.trim().to_string(), v.trim().to_string());
                }
            }
        }
        Self { entries }
    }

    /// Resolve a key with optional simple variable substitution using { $var } placeholders.
    pub fn get(&self, key: &str, args: Option<&HashMap<&str, String>>) -> String {
        if let Some(v) = self.entries.get(key) {
            if let Some(map) = args {
                let mut out = v.clone();
                for (k, val) in map {
                    // Replace occurrences like { $key }
                    let needle = format!("{{ ${} }}", k);
                    out = out.replace(&needle, val);
                }
                return out;
            }
            return v.clone();
        }
        // Fallback to key itself if missing
        key.to_string()
    }
}

/// Convenience API to localize a message key using a language code.
pub fn localize(lang: &str, key: &str, args: Option<&HashMap<&str, String>>) -> String {
    let cat = I18nCatalog::load(lang);
    cat.get(key, args)
}


