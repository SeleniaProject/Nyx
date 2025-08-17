//! Nyx FEC utilities.
//! - Fixed-size (1280B) shard packing helpers.
//! - Reed-Solomon (GF(2^8)) erasure coding wrappers specialized for 1280B shards.
//! - Lightweight timing helpers.
//! - Optional adaptive redundancy helper behind the `raptorq` feature.
//!
//! This crate avoids unsafe code and external C/C++ backends by default.

#![forbid(unsafe_code)]

#[cfg(feature = "raptorq")]
pub mod raptorq;
pub mod timing;
pub mod padding;
pub mod rs1280;

/// Error type for FEC operations in this crate.
#[derive(Debug)]
pub enum Error { Protocol(String) }
impl core::fmt::Display for Error {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self { Error::Protocol(s) => write!(f, "{s}") }
	}
}
impl std::error::Error for Error {}

/// Convenience result alias.
pub type Result<T> = core::result::Result<T, Error>;

