#![forbid(unsafe_code)]

//! Nyx SDK — Application-facing API surface
//!
//! - Error/Result type_s (`nyx_sdk::Error` / `nyx_sdk::Result<T>`)
//! - Lightweight stream API for app_s (`NyxStream`)
//! - Daemon IPC client (JSON over Unix Domain Socket / Window_s Named Pipe)
//! - JSON model_s that mirror future gRPC/prost type_s (`proto`)
//!
//! Designed to minimize dependencie_s and integrate cleanly with `nyx-stream` and `nyx-core`.

pub mod error;
pub mod config;
pub mod event_s;
pub mod daemon;
pub mod stream;
pub mod reconnect;
pub mod retry;
pub mod proto;

pub use error::{Error, Result};
pub use config::SdkConfig;
pub use event_s::Event;
pub use stream::NyxStream;
pub use daemon::DaemonClient;
pub use proto a_s api;

