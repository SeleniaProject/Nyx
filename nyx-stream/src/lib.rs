
#![forbid(unsafe_code)]

pub mod errors;
pub mod frame;
pub mod flow_controller;
pub mod builder;
pub mod multipath;
pub mod async_stream;
pub mod frame_codec;
pub mod congestion;
pub mod plugin;            // Plugin core types
pub mod plugin_registry;   // In-memory registry and permissions
pub mod plugin_cbor;       // CBOR header parsing helpers
pub mod plugin_frame;      // Full plugin frame (CBOR-serializable)
pub mod plugin_dispatch;   // Dispatcher for plugin frames
pub mod plugin_handshake;  // Handshake helpers and types
pub mod plugin_ipc;        // IPC helper traits (stubs for now)
pub mod plugin_sandbox;    // Cooperative sandbox (policy + guards)
pub mod plugin_manifest;   // Manifest loader (TOML)
pub mod plugin_settings;   // Runtime settings for plugins
pub mod hpke_rekey;        // Rekey trigger helpers (tests/integration use)

pub use errors::{Error, Result};
pub use frame::{Frame, FrameHeader, FrameType};
pub use frame_codec::FrameCodec;

