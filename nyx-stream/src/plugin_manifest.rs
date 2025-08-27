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
    pub permissions: Vec<Permission>,
}

impl PluginManifest {
    pub fn plugin_id(&self) -> PluginId {
        PluginId(self.id)
    }

    pub fn to_info(&self) -> PluginInfo {
        PluginInfo::new(
            self.plugin_id(),
            self.name.clone(),
            self.permissions.clone(),
        )
    }

    pub async fn register_into(
        &self,
        reg: &crate::plugin_registry::PluginRegistry,
    ) -> Result<(), &'static str> {
        reg.register(self.to_info()).await
    }
}

pub fn load_manifest_from_toml_str(s: &str) -> Result<PluginManifest, String> {
    toml::from_str::<PluginManifest>(s).map_err(|e| e.to_string())
}

pub fn validate_manifest(m: &PluginManifest) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if m.id == 0 {
        errors.push("Plugin ID must be greater than 0".to_string());
    }

    if m.name.is_empty() {
        errors.push("Plugin name cannot be empty".to_string());
    }

    if m.name.len() > 64 {
        errors.push("Plugin name cannot exceed 64 characters".to_string());
    }

    if m.version == 0 {
        errors.push("Manifest version must be greater than 0".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_manifest_minimal() -> Result<(), Box<dyn std::error::Error>> {
        let t = r#"
id = 10
name = "geo"
version = 1
"#;
        let m = load_manifest_from_toml_str(t)?;
        assert_eq!(m.plugin_id(), PluginId(10));
        assert_eq!(m.name, "geo");
        assert_eq!(m.version, 1);
        assert!(m.permissions.is_empty());
        Ok(())
    }

    #[test]
    fn parse_manifest_with_permissions() -> Result<(), Box<dyn std::error::Error>> {
        let t = r#"
id = 11
name = "io"
version = 1
permissions = ["handshake", "data_access"]
"#;
        let m = load_manifest_from_toml_str(t)?;
        assert_eq!(m.permissions.len(), 2);
        assert!(validate_manifest(&m).is_ok());
        let info = m.to_info();
        assert_eq!(info.id, PluginId(11));
        assert!(info.permissions.contains(&Permission::Handshake));
        Ok(())
    }

    #[test]
    fn invalid_manifest_fails_validation() -> Result<(), Box<dyn std::error::Error>> {
        let t = r#"
id = 0
name = ""
version = 0
"#;
        let m = load_manifest_from_toml_str(t)?;
        let errors = validate_manifest(&m).unwrap_err();
        assert!(!errors.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn register_into_registry_works() -> Result<(), Box<dyn std::error::Error>> {
        let reg = crate::plugin_registry::PluginRegistry::new();
        let t = r#"
id = 12
name = "ctrl"
version = 1
permissions = ["control"]
"#;
        let m = load_manifest_from_toml_str(t)?;
        m.register_into(&reg).await?;
        assert!(reg.is_registered(PluginId(12)).await);
        Ok(())
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
