#![forbid(unsafe_code)]

//! Nyx SDK  EApplication-facing API surface
//!
//! - Error/Result type_s (`nyx_sdk::Error` / `nyx_sdk::Result<T>`)
//! - Lightweight stream API for app_s (`NyxStream`)
//! - Daemon IPC client (JSON over Unix Domain Socket / windows Named Pipe)
//! - JSON model_s that mirror future gRPC/prost type_s (`proto`)
//!
//! Designed to minimize dependencie_s and integrate cleanly with `nyx-stream` and `nyx-core`.

pub mod config;
pub mod daemon;
pub mod error;
pub mod events;
pub mod proto;
pub mod reconnect;
pub mod retry;
pub mod stream;

pub use config::SdkConfig;
pub use daemon::DaemonClient;
pub use error::{Error, Result};
pub use events::Event;
pub use proto as api;
pub use stream::NyxStream;
