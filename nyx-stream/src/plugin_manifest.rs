use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::plugin::PluginId;
use crate::plugin_registry::{Permission, PluginInfo};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub id: u32,
    pub name: String,
    pub version: u16,
    #[serde(default)]
    pub permission_s: Vec<Permission>,
}

impl PluginManifest {
    pub fn plugin_id(&self) -> PluginId {
        PluginId(self.id)
    }

    pub fn to_info(&self) -> PluginInfo {
        PluginInfo::new(
            self.plugin_id(),
            self.name.clone(),
            self.permission_s.clone(),
        )
    }

    pub async fn register_into(
        &self,
        reg: &crate::plugin_registry::PluginRegistry,
    ) -> Result<(), &'static str> {
        reg.register(self.to_info()).await
    }
}

pub fn load_manifest_from_toml_str(_s: &str) -> Result<PluginManifest, String> {
    toml::from_str::<PluginManifest>(_s).map_err(|e| e.to_string())
}

pub fn validate_manifest(__m: &PluginManifest) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if __m.id == 0 {
        errors.push("Plugin ID must be greater than 0".to_string());
    }

    if __m.name.is_empty() {
        errors.push("Plugin name cannot be empty".to_string());
    }

    if __m.name.len() > 64 {
        errors.push("Plugin name cannot exceed 64 characters".to_string());
    }

    if __m.version == 0 {
        errors.push("Manifest version must be greater than 0".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn parse_manifest_minimal() -> Result<(), Box<dyn std::error::Error>> {
        let __t = r#"
id = 10
name = "geo"
version = 1
"#;
        let __m = load_manifest_from_toml_str(__t)?;
        assert_eq!(__m.plugin_id(), PluginId(10));
        assert_eq!(__m._name, "geo");
        assert_eq!(__m.version, 1);
        assert!(__m.permission_s.is_empty());
        Ok(())
    }

    #[test]
    fn parse_manifest_with_permission_s() -> Result<(), Box<dyn std::error::Error>> {
        let __t = r#"
id = 11
name = "io"
version = 1
permission_s = ["handshake", "data_acces_s"]
"#;
        let __m = load_manifest_from_toml_str(__t)?;
        assert_eq!(__m.permission_s.len(), 2);
        assert!(validate_manifest(&__m).is_ok());
        let info = __m.to_info();
        assert_eq!(info.__id, PluginId(11));
        assert!(info.permission_s.contains(&Permission::Handshake));
        Ok(())
    }

    #[test]
    fn invalid_manifest_fails_validation() -> Result<(), Box<dyn std::error::Error>> {
        let __t = r#"
id = 0
name = ""
version = 0
"#;
        let __m = load_manifest_from_toml_str(__t)?;
        let err_s = validate_manifest(&__m).unwrap_err();
        assert!(!err_s.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn register_into_registry_work_s() -> Result<(), Box<dyn std::error::Error>> {
        let reg = crate::plugin_registry::PluginRegistry::new();
        let __t = r#"
id = 12
name = "ctrl"
version = 1
permission_s = ["control"]
"#;
        let __m = load_manifest_from_toml_str(__t)?;
        __m.register_into(&reg).await?;
        assert!(reg.is_registered(PluginId(12)).await);
        Ok(())
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let __t = r#"
id = 13
name = "x"
version = 1
permission_s = []
extra = "nope"
"#;
        let err = load_manifest_from_toml_str(__t).unwrap_err();
        assert!(err.contains("unknown field"));
    }
}
