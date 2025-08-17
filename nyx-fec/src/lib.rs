
#![forbid(unsafe_code)]

pub mod raptorq;
pub mod timing;
pub mod padding;

#[derive(Debug)]
pub enum Error { Protocol(String) }
impl core::fmt::Display for Error {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self { Error::Protocol(s) => write!(f, "{s}") }
	}
}
impl std::error::Error for Error {}

pub type Result<T> = core::result::Result<T, Error>;

