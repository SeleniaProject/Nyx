//! WASM-specific error handling for Nyx SDK

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

/// WASM-compatible error types for Nyx operations
#[derive(Debug, Clone)]
pub enum NyxWasmError {
    /// Cryptographic operation failed
    CryptographicError(String),
    /// Network operation failed
    NetworkError(String),
    /// Configuration is invalid
    ConfigurationError(String),
    /// Push registration failed
    PushRegistrationError(String),
    /// Handshake failed
    HandshakeError(String),
    /// Serialization/deserialization error
    SerializationError(String),
    /// General operation error
    OperationError(String),
}

impl std::fmt::Display for NyxWasmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NyxWasmError::CryptographicError(msg) => write!(f, "Cryptographic error: {msg}"),
            NyxWasmError::NetworkError(msg) => write!(f, "Network error: {msg}"),
            NyxWasmError::ConfigurationError(msg) => write!(f, "Configuration error: {msg}"),
            NyxWasmError::PushRegistrationError(msg) => {
                write!(f, "Push registration error: {msg}")
            }
            NyxWasmError::HandshakeError(msg) => write!(f, "Handshake error: {msg}"),
            NyxWasmError::SerializationError(msg) => write!(f, "Serialization error: {msg}"),
            NyxWasmError::OperationError(msg) => write!(f, "Operation error: {msg}"),
        }
    }
}

impl std::error::Error for NyxWasmError {}

#[cfg(target_arch = "wasm32")]
impl From<NyxWasmError> for JsValue {
    fn from(error: NyxWasmError) -> Self {
        JsValue::from_str(&error.to_string())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<NyxWasmError> for crate::JsValue {
    fn from(error: NyxWasmError) -> Self {
        crate::JsValue::from_string(&error.to_string())
    }
}

/// Result type for WASM operations
#[cfg(target_arch = "wasm32")]
pub type WasmResult<T> = Result<T, JsValue>;

#[cfg(not(target_arch = "wasm32"))]
pub type WasmResult<T> = Result<T, crate::JsValue>;

/// Convert a standard Result to WasmResult
pub fn to_wasm_result<T, E: std::fmt::Display>(result: Result<T, E>) -> WasmResult<T> {
    result.map_err(|e| NyxWasmError::OperationError(e.to_string()).into())
}
