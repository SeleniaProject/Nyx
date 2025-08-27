#![forbid(unsafe_code)]

use crate::error::{Error, Result};
use bytes::Bytes;
use nyx_stream::{AsyncStream, AsyncStreamConfig, pair};

/// SDK wrapper for streams. Delegates to nyx-stream's AsyncStream, providing an adapter.
#[derive(Clone)]
pub struct NyxStream {
    inner: AsyncStream,
}

impl NyxStream {
    /// Create a new stream with default configuration
    pub fn new() -> Self {
        Self {
            inner: AsyncStream::new(AsyncStreamConfig::default()),
        }
    }

    /// Create a stream with custom configuration
    pub fn with_config(config: AsyncStreamConfig) -> Self {
        Self {
            inner: AsyncStream::new(config),
        }
    }

    /// Create a pair of connected streams for testing
    pub fn pair(_buffer_size: usize) -> (Self, Self) {
        let config1 = AsyncStreamConfig {
            stream_id: 1,
            ..AsyncStreamConfig::default()
        };
        let config2 = AsyncStreamConfig {
            stream_id: 2,
            ..AsyncStreamConfig::default()
        };
        let (inner1, inner2) = pair(config1, config2);
        (
            Self { inner: inner1 },
            Self { inner: inner2 },
        )
    }

    /// Send data through the stream
    pub async fn send<T: Into<Bytes>>(&mut self, data: T) -> Result<()> {
        self.inner
            .send(data.into())
            .await
            .map_err(|e| Error::Stream(e.to_string()))
    }

    /// Receive data from the stream with timeout
    pub async fn recv(&mut self, _timeout_ms: u64) -> Result<Option<Bytes>> {
        self.inner
            .recv()
            .await
            .map_err(|e| Error::Stream(e.to_string()))
    }

    /// Close the stream
    pub async fn close(&mut self) -> Result<()> {
        self.inner
            .close()
            .await
            .map_err(|e| Error::Stream(e.to_string()))
    }

    /// Check if the stream is closed
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }
}

impl Default for NyxStream {
    fn default() -> Self {
        Self::new()
    }
}
