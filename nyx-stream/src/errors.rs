use thiserror::Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("cbor: {0}")]
    Cbor(#[from] ciborium::de::Error<std::io::Error>),
    #[error("cbor-ser: {0}")]
    CborSer(#[from] ciborium::ser::Error<std::io::Error>),
    #[error("config: {0}")]
    Config(String),
    #[error("protocol: {0}")]
    Protocol(String),
    #[error("timeout")]
    Timeout,
    #[error("channel closed")]
    ChannelClosed,
}

impl Error {
    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(msg.into())
    }
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
}
