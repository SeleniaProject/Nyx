#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// For non-WASM environments, define a placeholder JsValue
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
pub struct JsValue(#[allow(dead_code)] String);

#[cfg(not(target_arch = "wasm32"))]
impl JsValue {
    pub fn from_string(s: &str) -> Self {
        JsValue(s.to_string())
    }
}

pub mod multipath;

#[cfg(target_arch = "wasm32")]
pub use multipath::MultipathManager;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn init() {
    // Initialize logging for WASM environment
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::prelude::*;
        web_sys::console::log_1(&"Nyx WASM SDK initialized".into());
    }
}

/// Get SDK version information
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Initialize SDK with configuration
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn init_with_config(config: &str) -> Result<(), JsValue> {
    init();
    
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::console::log_1(&format!("Nyx SDK initialized with config: {}", config).into());
    }
    
    // Parse and validate configuration
    let _config: serde_json::Value = serde_json::from_str(config)
        .map_err(|e| JsValue::from_string(&format!("Invalid config JSON: {e}")))?;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        init();
        // Basic smoke test - should not panic
    }

    #[test]
    fn test_version() {
        let version = version();
        assert!(!version.is_empty());
    }

    #[test]
    fn test_init_with_valid_config() {
        let config = r#"{"multipath": {"enabled": true}}"#;
        assert!(init_with_config(config).is_ok());
    }

    #[test]
    fn test_init_with_invalid_config() {
        let config = "invalid json";
        assert!(init_with_config(config).is_err());
    }
}
