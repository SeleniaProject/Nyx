use thiserror::Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
	#[error("io: {0}")]
	Io(#[from] std::io::Error),
	#[error("serde: {0}")]
	Serde(#[from] serde_json::Error),
	#[error("config: {0}")]
	Config(String),
	#[error("protocol: {0}")]
	Protocol(String),
}

impl Error {
	pub fn config(msg: impl Into<String>) -> Self { Self::Config(msg.into()) }
	pub fn protocol(msg: impl Into<String>) -> Self { Self::Protocol(msg.into()) }
}
