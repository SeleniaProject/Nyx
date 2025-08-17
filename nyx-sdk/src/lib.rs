#![forbid(unsafe_code)]

//! Nyx SDK — Application-facing API surface
//!
//! - Error/Result types (`nyx_sdk::Error` / `nyx_sdk::Result<T>`)
//! - Lightweight stream API for apps (`NyxStream`)
//! - Daemon IPC client (JSON over Unix Domain Socket / Windows Named Pipe)
//! - JSON models that mirror future gRPC/prost types (`proto`)
//!
//! Designed to minimize dependencies and integrate cleanly with `nyx-stream` and `nyx-core`.

pub mod error;
pub mod config;
pub mod events;
pub mod daemon;
pub mod stream;
pub mod reconnect;
pub mod retry;
pub mod proto;

pub use error::{Error, Result};
pub use config::SdkConfig;
pub use events::Event;
pub use stream::NyxStream;
pub use daemon::DaemonClient;
pub use proto as api;

