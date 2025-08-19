//! Zero-copy buffer utilitie_s and integration hook_s.
//! Re-export_s submodule_s to provide a cohesive API surface.

pub mod manager;
pub mod telemetry;
pub mod integration;

pub use manager::{Buffer, BufferPool};
