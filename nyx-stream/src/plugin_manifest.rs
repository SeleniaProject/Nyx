#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use schemars::{JsonSchema, schema_for};

use crate::plugin::PluginId;
use crate::plugin_registry::{Permission, PluginInfo};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
	#[schemars(description = "Unique plugin identifier", range(min = 1))]
	pub id: u32,
	#[schemars(description = "Human-readable name", length(min = 1, max = 64))]
	pub name: String,
	#[schemars(description = "Manifest schema version", range(min = 1))]
	pub version: u16,
	#[serde(default)]
	pub permissions: Vec<Permission>,
}

impl PluginManifest {
	pub fn plugin_id(&self) -> PluginId { PluginId(self.id) }

	pub fn to_info(&self) -> PluginInfo {
		PluginInfo::new(self.plugin_id(), self.name.clone(), self.permissions.clone())
	}

	pub async fn register_into(&self, reg: &crate::plugin_registry::PluginRegistry) -> Result<(), &'static str> {
		reg.register(self.to_info()).await
	}
}

pub fn load_manifest_from_toml_str(s: &str) -> Result<PluginManifest, String> {
	toml::from_str::<PluginManifest>(s).map_err(|e| e.to_string())
}

pub fn validate_manifest(m: &PluginManifest) -> Result<(), Vec<String>> {
	let schema = schema_for!(PluginManifest);
	let schema_json = serde_json::to_value(&schema).unwrap_or(serde_json::json!({}));
	let compiled = match jsonschema::JSONSchema::compile(&schema_json) {
		Ok(c) => c,
		Err(e) => return Err(vec![format!("schema compile error: {e}")]),
	};
	let val = serde_json::to_value(m).unwrap_or(serde_json::json!({}));
	let mut errs = Vec::new();
	if let Err(iter) = compiled.validate(&val) {
		for e in iter { errs.push(e.to_string()); }
	}
	if errs.is_empty() { Ok(()) } else { Err(errs) }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_manifest_minimal() {
		let t = r#"
id = 10
name = "geo"
version = 1
"#;
		let m = load_manifest_from_toml_str(t).unwrap();
		assert_eq!(m.plugin_id(), PluginId(10));
		assert_eq!(m.name, "geo");
		assert_eq!(m.version, 1);
		assert!(m.permissions.is_empty());
	}

	#[test]
	fn parse_manifest_with_permissions() {
		let t = r#"
id = 11
name = "io"
version = 1
permissions = ["handshake", "data_access"]
"#;
		let m = load_manifest_from_toml_str(t).unwrap();
		assert_eq!(m.permissions.len(), 2);
	assert!(validate_manifest(&m).is_ok());
	let info = m.to_info();
	assert_eq!(info.id, PluginId(11));
	assert!(info.permissions.contains(&Permission::Handshake));
	}

	#[test]
	fn invalid_manifest_fails_validation() {
		let t = r#"
id = 0
name = ""
version = 0
"#;
		let m = load_manifest_from_toml_str(t).unwrap();
		let errs = validate_manifest(&m).unwrap_err();
		assert!(!errs.is_empty());
	}

	#[tokio::test]
	async fn register_into_registry_works() {
		let reg = crate::plugin_registry::PluginRegistry::new();
		let t = r#"
id = 12
name = "ctrl"
version = 1
permissions = ["control"]
"#;
		let m = load_manifest_from_toml_str(t).unwrap();
		m.register_into(&reg).await.unwrap();
		assert!(reg.is_registered(PluginId(12)).await);
	}

	#[test]
	fn unknown_fields_are_rejected() {
		let t = r#"
id = 13
name = "x"
version = 1
permissions = []
extra = "nope"
"#;
		let err = load_manifest_from_toml_str(t).unwrap_err();
		assert!(err.contains("unknown field"));
	}
}
