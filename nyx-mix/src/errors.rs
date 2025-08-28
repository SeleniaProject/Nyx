/// Error types for Nyx Mix module
use thiserror::Error;

pub type Result<T, E = MixError> = core::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum MixError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("config: {0}")]
    Config(String),
    #[error("cover traffic: {0}")]
    CoverTraffic(String),
    #[error("adaptation: {0}")]
    Adaptation(String),
    #[error("anonymity: {0}")]
    Anonymity(String),
    #[error("internal: {msg}")]
    Internal { msg: String },
}

impl MixError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn cover_traffic(msg: impl Into<String>) -> Self {
        Self::CoverTraffic(msg.into())
    }

    pub fn adaptation(msg: impl Into<String>) -> Self {
        Self::Adaptation(msg.into())
    }

    pub fn anonymity(msg: impl Into<String>) -> Self {
        Self::Anonymity(msg.into())
    }
}
