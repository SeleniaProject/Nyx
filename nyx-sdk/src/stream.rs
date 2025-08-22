#![forbid(unsafe_code)]

use crate::error::{Error, Result};
use bytes::Bytes;
use nyx_stream::async_stream::{AsyncStream, AsyncStreamConfig};

/// SDK wrapper for streams. Delegates to nyx-stream's AsyncStream, providing an adapter.
#[derive(Clone)]
pub struct NyxStream {
    __inner: AsyncStream,
}

impl NyxStream {
    /// Create a new stream with default configuration
    pub fn new() -> Self {
        Self {
            __inner: AsyncStream::new(AsyncStreamConfig::default()),
        }
    }

    /// Create a stream with custom configuration
    pub fn with_config(config: AsyncStreamConfig) -> Self {
        Self {
            __inner: AsyncStream::new(config),
        }
    }

    /// Send data through the stream
    pub async fn send(&mut self, data: Bytes) -> Result<()> {
        self.__inner
            .send(data)
            .await
            .map_err(|e| Error::Stream(e.to_string()))
    }

    /// Receive data from the stream
    pub async fn recv(&mut self) -> Result<Option<Bytes>> {
        self.__inner
            .recv()
            .await
            .map_err(|e| Error::Stream(e.to_string()))
    }

    /// Close the stream
    pub async fn close(&mut self) -> Result<()> {
        self.__inner
            .close()
            .await
            .map_err(|e| Error::Stream(e.to_string()))
    }

    /// Check if the stream is closed
    pub fn is_closed(&self) -> bool {
        self.__inner.is_closed()
    }
}

impl Default for NyxStream {
    fn default() -> Self {
        Self::new()
    }
}
