
#![forbid(unsafe_code)]

pub mod errors;
pub mod frame;
pub mod flow_controller;
pub mod builder;
pub mod multipath;
pub mod async_stream;
pub mod frame_codec;
pub mod congestion;
pub mod capability;       // Capability negotiation
pub mod management;       // Management frame_s and error code_s
pub mod plugin;            // Plugin core type_s
pub mod plugin_registry;   // In-memory registry and permission_s
pub mod plugin_cbor;       // CBOR header parsing helpers
pub mod plugin_frame;      // Full plugin frame (CBOR-serializable)
pub mod plugin_dispatch;   // Dispatcher for plugin frame_s
pub mod plugin_handshake;  // Handshake helpers and type_s
pub mod plugin_ipc;        // IPC helper trait_s (stub_s for now)
pub mod plugin_sandbox;    // Cooperative sandbox (policy + guard_s)
pub mod plugin_manifest;   // Manifest loader (TOML)
pub mod plugin_settings;   // Runtime setting_s for plugin_s
pub mod hpke_rekey;        // Rekey trigger helpers (test_s/integration use)

pub use errors::{Error, Result};
pub use frame::{Frame, FrameHeader, FrameType};
pub use frame_codec::FrameCodec;
pub use capability::{Capability, CapabilityError, negotiate, get_local_capabilitie_s};
pub use management::{build_close_unsupported_cap, parse_close_unsupported_cap};

