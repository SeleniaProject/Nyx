use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AppSettings {
	/// Example application setting: log level (info, debug, warn, error)
	#[serde(default = "default_level")] pub log_level: String,
	/// Optional endpoint of rendezvous server
	#[serde(default)] pub rendezvous_url: Option<String>,
}

fn default_level() -> String { "info".to_string() }

impl Default for AppSettings {
	fn default() -> Self { Self { log_level: default_level(), rendezvous_url: None } }
}

#[derive(thiserror::Error, Debug)]
pub enum SettingsError {
	#[error("invalid json: {0}")] InvalidJson(String),
	#[error("schema violation: {0}")] Schema(String),
}

pub type Result<T> = std::result::Result<T, SettingsError>;

/// Validate JSON string against AppSettings schema and deserialize.
pub fn validate_and_parse(json: &str) -> Result<AppSettings> {
	let v: serde_json::Value = serde_json::from_str(json).map_err(|e| SettingsError::InvalidJson(e.to_string()))?;
	let schema = schemars::schema_for!(AppSettings);
	let compiled = jsonschema::JSONSchema::options().with_draft(jsonschema::Draft::Draft7).compile(&serde_json::to_value(schema.schema).unwrap()).unwrap();
	if let Err(errors) = compiled.validate(&v) {
		let joined = errors.map(|e| e.to_string()).collect::<Vec<_>>().join("; ");
		return Err(SettingsError::Schema(joined));
	}
	serde_json::from_value(v).map_err(|e| SettingsError::InvalidJson(e.to_string()))
}

/// Versioned settings blob to support sync/merge policies.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionedSettings<T> {
	pub version: u64,
	pub data: T,
}

impl<T> VersionedSettings<T> {
	pub fn new(version: u64, data: T) -> Self { Self { version, data } }
}
