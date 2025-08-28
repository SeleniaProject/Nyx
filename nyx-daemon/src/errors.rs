/// Error types for Nyx Daemon module
use thiserror::Error;

pub type Result<T, E = DaemonError> = core::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("config: {0}")]
    Config(String),
    #[error("internal: {0}")]
    Internal(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("transport: {0}")]
    Transport(String),
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("path build error: {0}")]
    PathBuild(#[from] PathBuildError),
    #[error("resource exhaustion")]
    ResourceExhaustion,
}

#[derive(Debug, Error)]
pub enum PathBuildError {
    #[error("network unreachable")]
    NetworkUnreachable,
    #[error("connection timeout")]
    ConnectionTimeout,
    #[error("handshake failed")]
    HandshakeFailure,
    #[error("insufficient bandwidth")]
    InsufficientBandwidth,
    #[error("high latency")]
    HighLatency,
    #[error("packet loss")]
    PacketLoss,
    #[error("authentication error")]
    AuthenticationError,
    #[error("protocol mismatch")]
    ProtocolMismatch,
    #[error("resource exhaustion")]
    ResourceExhaustion,
    #[error("unknown error: {0}")]
    Unknown(String),
}

impl DaemonError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }

    pub fn transport(msg: impl Into<String>) -> Self {
        Self::Transport(msg.into())
    }

    pub fn not_implemented(msg: impl Into<String>) -> Self {
        Self::NotImplemented(msg.into())
    }
}
