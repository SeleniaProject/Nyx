#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fs;

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

    /// Resolve a key with variable substitution and basic formatting.
    /// Supported patterns inside strings:
    /// - { $var }                      → simple substitution
    /// - { $var | upper }              → uppercase
    /// - { $var | lower }              → lowercase
    /// - { $var | trim }               → trim whitespace
    /// - { $var | truncate:10 }        → truncate to N chars (append … if trimmed)
    /// - { $var or 'default' }         → fallback when var missing
    pub fn get(&self, key: &str, args: Option<&HashMap<&str, String>>) -> String {
        let template = match self.entries.get(key) {
            Some(v) => v.as_str(),
            None => return key.to_string(),
        };

        let mut out = String::with_capacity(template.len());
        let bytes = template.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'{' {
                // Try to find matching '}'
                if let Some(close) = template[i+1..].find('}') {
                    let inner = &template[i+1..i+1+close];
                    let replaced = Self::render_placeholder(inner.trim(), args);
                    out.push_str(&replaced);
                    i += close + 2; // skip '...}'
                    continue;
                }
            }
            out.push(bytes[i] as char);
            i += 1;
        }
        out
    }

    fn render_placeholder(spec: &str, args: Option<&HashMap<&str, String>>) -> String {
        // Expected forms:
        // "$var", "$var | upper", "$var or 'default'", "$var | truncate:10"
        // Strip optional leading/trailing braces handled by caller.
        let mut var_part = spec.trim();
        let mut fallback: Option<String> = None;
        // Handle fallback: split by " or " outside quotes
        if let Some(idx) = var_part.find(" or ") {
            let (lhs, rhs) = var_part.split_at(idx);
            var_part = lhs.trim();
            let rhs = rhs.trim_start_matches(" or ").trim();
            // Accept quoted 'text' or "text"; otherwise take as raw
            let fb = if (rhs.starts_with('\'') && rhs.ends_with('\'')) || (rhs.starts_with('"') && rhs.ends_with('"')) {
                rhs[1..rhs.len()-1].to_string()
            } else { rhs.to_string() };
            fallback = Some(fb);
        }

        // Split modifiers: "$var | upper" or "$var | truncate:10"
        let mut parts = var_part.split('|').map(|s| s.trim());
        let var_token = parts.next().unwrap_or("");
        let mut value = if let Some(name) = var_token.strip_prefix('$') {
            if let Some(map) = args { map.get(name).cloned() } else { None }
        } else { None };

        if value.is_none() {
            if let Some(fb) = fallback { return fb; }
            return format!("{{{}}}", spec); // leave as-is
        }

        let mut v = value.take().unwrap();
        for modifier in parts {
            if modifier.eq_ignore_ascii_case("upper") { v = v.to_uppercase(); continue; }
            if modifier.eq_ignore_ascii_case("lower") { v = v.to_lowercase(); continue; }
            if modifier.eq_ignore_ascii_case("trim") { v = v.trim().to_string(); continue; }
            if let Some(rest) = modifier.strip_prefix("truncate:") {
                if let Ok(n) = rest.trim().parse::<usize>() {
                    if v.chars().count() > n {
                        let truncated: String = v.chars().take(n).collect();
                        v = format!("{}…", truncated);
                    }
                    continue;
                }
            }
            // Unknown modifier: keep literal
        }
        v
    }
}

/// Convenience API to localize a message key using a language code.
pub fn localize(lang: &str, key: &str, args: Option<&HashMap<&str, String>>) -> String {
    let cat = I18nCatalog::load(lang);
    cat.get(key, args)
}


