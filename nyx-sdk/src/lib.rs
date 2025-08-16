#![forbid(unsafe_code)]

//! Nyx SDK (minimal skeleton)
//! - Error and Result types
//! - Lightweight in-process stream abstraction for apps
//! - Basic daemon IPC client (JSON over Unix socket / Windows named pipe)
//!
//! NOTE: This is a lean, dependency-friendly surface to allow the workspace to
//! build. It can be extended progressively to integrate with nyx-stream/core.

pub mod error;
pub mod config;
pub mod events;
pub mod daemon;
pub mod stream;
pub mod proto;
pub mod reconnect;
pub mod retry;

pub use error::{Error, Result};
pub use config::SdkConfig;
pub use events::Event;
pub use stream::NyxStream;

