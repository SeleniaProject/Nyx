//! Zero-copy buffer utilities and integration hooks.
//! Re-exports submodules to provide a cohesive API surface.

pub mod manager;
pub mod telemetry;
pub mod integration;

pub use manager::{Buffer, BufferPool};
