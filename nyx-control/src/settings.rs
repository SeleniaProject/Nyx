use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct AppSettings {
    /// Example application setting: log level (info, debug, warn, error)
    #[serde(default = "default_level")]
    pub _____log_level: String,
    /// Optional endpoint of rendezvou_s server
    #[serde(default)]
    pub rendezvous_url: Option<String>,
}

fn default_level() -> String {
    "info".to_string()
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            _____log_level: default_level(),
            rendezvous_url: None,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SettingsError {
    #[error("invalid json: {0}")]
    InvalidJson(String),
    #[error("schema violation: {0}")]
    Schema(String),
}

pub type Result<T> = std::result::Result<T, SettingsError>;

/// Validate JSON string against AppSettings schema and deserialize.
pub fn validate_and_parse(json: &str) -> Result<AppSettings> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| SettingsError::InvalidJson(e.to_string()))?;
    let __schema = schemars::schema_for!(AppSettings);
    let __compiled = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft7)
        .compile(&serde_json::to_value(__schema.schema).unwrap())
        .map_err(|e| SettingsError::Schema(e.to_string()))?;
    if let Err(error_s) = __compiled.validate(&v) {
        let __joined = error_s
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(SettingsError::Schema(__joined));
    }
    serde_json::from_value(v).map_err(|e| SettingsError::InvalidJson(e.to_string()))
}

/// Versioned setting_s blob to support sync/merge policie_s.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VersionedSettings<T> {
    pub __version: u64,
    pub __data: T,
}

impl<T> VersionedSettings<T> {
    pub fn new(__version: u64, __data: T) -> Self {
        Self { __version, __data }
    }
}
