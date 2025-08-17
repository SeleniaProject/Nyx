use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use std::{borrow::Cow, collections::HashMap};
use unic_langid::LanguageIdentifier;

/// Minimal i18n helper around `fluent-bundle`.
/// This avoids I/O and expects resources to be provided by the caller.
#[derive(Default)]
pub struct I18n {
	bundles: HashMap<String, FluentBundle<FluentResource>>,
}

impl I18n {
	/// Insert a Fluent resource for a given language tag (e.g. "en-US").
	pub fn insert_resource(&mut self, lang: &str, ftl: &str) -> anyhow::Result<()> {
		let res = FluentResource::try_new(ftl.to_string()).map_err(|(_, e)| anyhow::anyhow!("fluent parse error: {e:?}"))?;
		let langid: LanguageIdentifier = lang.parse().unwrap_or_else(|_| "en-US".parse().expect("valid fallback"));
	let mut bundle = FluentBundle::new(vec![langid]);
	// Avoid adding Unicode isolation marks around interpolations for simple UIs
	bundle.set_use_isolating(false);
		bundle.add_resource(res).map_err(|e| anyhow::anyhow!("bundle add resource: {e:?}"))?;
		self.bundles.insert(lang.to_string(), bundle);
		Ok(())
	}

	/// Format a message with optional arguments for the requested language.
	/// Falls back to key if not found.
	pub fn format<'a>(&'a self, lang: &str, key: &str, args: Option<&FluentArgs<'a>>) -> String {
		let Some(bundle) = self.bundles.get(lang) else { return key.to_string() };
		let Some(msg) = bundle.get_message(key) else { return key.to_string() };
		let Some(pattern) = msg.value() else { return key.to_string() };
	let mut errors = vec![];
	bundle.format_pattern(pattern, args, &mut errors).to_string()
	}

	/// Convenience for formatting with owned map-like args.
	pub fn format_kv(&self, lang: &str, key: &str, kv: &[(&str, &str)]) -> String {
		let mut args = FluentArgs::new();
	for (k, v) in kv { args.set(*k, FluentValue::String(Cow::Owned((*v).to_string()))); }
		self.format(lang, key, Some(&args))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn basic_i18n() {
		let mut i = I18n::default();
		i.insert_resource("en-US", "hello = Hello, { $name }!\n").unwrap();
		let s = i.format_kv("en-US", "hello", &[("name", "Nyx")]);
		assert_eq!(s, "Hello, Nyx!");
		// missing lang/key => fallback
		assert_eq!(i.format("ja", "missing", None), "missing");
	}
}
