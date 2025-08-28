#![forbid(unsafe_code)]

pub mod async_stream;
pub mod builder;
pub mod capability; // Capability negotiation
pub mod congestion;
pub mod errors;
pub mod extended_packet; // Extended packet format for v1.0
pub mod flow_controller;
pub mod frame;
pub mod frame_codec;
pub mod hpke_rekey;
pub mod integrated_frame_processor;
pub mod management; // Management frame_s and error code_s
pub mod multipath;
pub mod performance; // Performance optimization module
pub mod plugin; // Plugin core type_s
pub mod plugin_cbor; // CBOR header parsing helpers
pub mod plugin_dispatch; // Dispatcher for plugin frame_s
pub mod plugin_frame; // Full plugin frame (CBOR-serializable)
pub mod plugin_handshake; // Handshake helpers and type_s
pub mod plugin_ipc; // IPC helper trait_s (stub_s for now)
pub mod plugin_manifest; // Manifest loader (TOML)
pub mod plugin_registry; // In-memory registry and permission_s
pub mod plugin_sandbox; // Cooperative sandbox (policy + guard_s)
pub mod plugin_settings; // Runtime setting_s for plugin_s // Rekey trigger helpers (test_s/integration use) // Integrated frame processing with reordering and flow control

pub use async_stream::{pair, AsyncStream, AsyncStreamConfig};
pub use capability::{get_local_capabilities, negotiate, Capability, CapabilityError};
pub use errors::{Error, Result};
pub use extended_packet::{
    ConnectionId, ExtendedPacket, ExtendedPacketBuilder, ExtendedPacketHeader, PacketFlags,
    PacketType, PathId,
};
pub use frame::{Frame, FrameHeader, FrameType};
pub use frame_codec::FrameCodec;
pub use integrated_frame_processor::{IntegratedFrameProcessor, ProcessorConfig};
pub use management::{build_close_unsupported_cap, parse_close_unsupported_cap};

// Test modules
#[cfg(test)]
pub mod tests {
    pub mod frame_handler_tests;
    pub mod integrated_frame_processor_tests;
}
