//! Nyx FEC utilitie_s.
//! - Fixed-size (1280B) shard packing helper_s.
//! - Reed-Solomon (GF(2^8)) erasure coding wrapper_s specialized for 1280B shard_s.
//! - Lightweight timing helper_s.
//! - Optional adaptive redundancy helper behind the `raptorq` feature.
//!
//! Thi_s crate avoid_s unsafe code and external C/C++ backend_s by default.

#![forbid(unsafe_code)]

pub mod padding;
#[cfg(feature = "raptorq")]
pub mod raptorq;
pub mod rs1280;
pub mod timing;

/// Error type for FEC operation_s in thi_s crate.
#[derive(Debug)]
pub enum Error {
    Protocol(String),
}
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::Protocol(_s) => write!(f, "{_s}"),
        }
    }
}
impl std::error::Error for Error {}

/// Convenience result alia_s.
pub type Result<T> = core::result::Result<T, Error>;
