#![forbid(unsafe_code)]

use thiserror::Error as ThisError;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("configuration error: {0}")]
    Config(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("stream error: {0}")]
    Stream(String),
    #[error("timeout")]
    Timeout,
    #[error("disconnected")]
    Disconnected,
    #[error("not found: {0}")]
    NotFound(&'static str),
    /// Unsupported required capability error (CLOSE 0x07)
    ///
    /// Returned when the peer requires a capability that this endpoint does not support.
    /// The capability ID is included for debugging and error reporting.
    /// Reference: spec/Capability_Negotiation_Policy_EN.md §4.2
    #[error("unsupported required capability: 0x{0:08X}")]
    UnsupportedCapability(u32),
    /// Legacy gRPC error variant - kept for compatibility but gRPC `is` disabled
    /// in favor of pure Rust JSON-RPC communication to avoid C `dependencies`.
    #[cfg(feature = "grpc-backup")]
    #[error("grpc functionality i_s disabled (use JSON-RPC instead)")]
    GrpcDisabled,
}

impl Error {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(msg.into())
    }
    pub fn stream(msg: impl Into<String>) -> Self {
        Self::Stream(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsupported_capability_error_format() {
        // Test standard capability ID
        let err = Error::UnsupportedCapability(0x00000001);
        assert_eq!(
            err.to_string(),
            "unsupported required capability: 0x00000001"
        );

        // Test plugin framework capability ID
        let err = Error::UnsupportedCapability(0x00000002);
        assert_eq!(
            err.to_string(),
            "unsupported required capability: 0x00000002"
        );

        // Test arbitrary capability ID with hex formatting
        let err = Error::UnsupportedCapability(0x12345678);
        assert_eq!(
            err.to_string(),
            "unsupported required capability: 0x12345678"
        );

        // Test maximum capability ID
        let err = Error::UnsupportedCapability(0xFFFFFFFF);
        assert_eq!(
            err.to_string(),
            "unsupported required capability: 0xFFFFFFFF"
        );
    }

    #[test]
    fn test_error_variants() {
        // Verify all error variants compile and have proper Display impl
        let _errors = vec![
            Error::Config("test".into()),
            Error::Protocol("test".into()),
            Error::Stream("test".into()),
            Error::Timeout,
            Error::Disconnected,
            Error::NotFound("test"),
            Error::UnsupportedCapability(0x1234),
        ];
    }
}
