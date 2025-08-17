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
	#[error("timeout")]
	Timeout,
	#[error("disconnected")]
	Disconnected,
	#[error("not found: {0}")]
	NotFound(&'static str),
	/// Legacy gRPC error variant - kept for compatibility but gRPC is disabled
	/// in favor of pure Rust JSON-RPC communication to avoid C dependencies.
	#[cfg(feature = "grpc-backup")]
	#[error("grpc functionality is disabled (use JSON-RPC instead)")]
	GrpcDisabled,
}

impl Error {
	pub fn config(msg: impl Into<String>) -> Self { Error::Config(msg.into()) }
	pub fn protocol(msg: impl Into<String>) -> Self { Error::Protocol(msg.into()) }
}

