pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Cryptographic error from nyx-crypto
    #[error("Cryptographic error: {0}")]
    Crypto(#[from] nyx_crypto::Error),
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
    #[error("invalid frame: {0}")]
    InvalidFrame(String),
    #[error("processing timeout")]
    ProcessingTimeout,
    #[error("stream error: {0}")]
    StreamError(String),
    #[error("multipath error: {message}")]
    MultipathError { message: String },
}

impl Error {
    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(msg.into())
    }
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
}
