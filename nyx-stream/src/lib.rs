#![forbid(unsafe_code)]
//! Nyx Secure Stream layer (skeleton)

pub mod errors;
pub mod flow_controller;
pub mod frame_handler;
pub mod integrated_frame_processor;
pub mod simple_frame_handler;

pub mod ack;
pub mod async_stream; // Re-enabled
pub mod builder;
pub mod congestion;
pub mod egress_zero_copy;
pub mod error_handler;
pub mod frame;
pub mod plugin;
#[cfg(feature = "plugin")]
mod plugin_ipc;
pub mod resource_manager;
pub mod state;
pub mod stream_frame;
pub mod tx;
pub mod zero_copy_tx;
// Always compile plugin_frame with internal cfgs so non-plugin builds can still parse/validate types minimally
#[cfg(feature = "plugin")]
pub mod plugin_cbor;
#[cfg(test)]
#[cfg(feature = "plugin")]
mod plugin_cbor_tests;
#[cfg(feature = "plugin")]
pub mod plugin_dispatch;
pub mod plugin_frame;
#[cfg(feature = "plugin")]
pub mod plugin_handshake;
#[cfg(feature = "plugin")]
pub mod plugin_settings;

mod cap_negotiator;
mod capability;
mod localized;
pub mod management;
#[cfg(feature = "plugin")]
mod plugin_geostat;
#[cfg(test)]
#[cfg(feature = "plugin")]
mod plugin_integration_test;
#[cfg(feature = "plugin")]
pub mod plugin_manifest;
#[cfg(feature = "plugin")]
mod plugin_registry;
#[cfg(feature = "dynamic_plugin")]
#[cfg_attr(target_os = "linux", path = "plugin_sandbox.rs")]
#[cfg_attr(target_os = "windows", path = "plugin_sandbox_windows.rs")]
#[cfg_attr(target_os = "macos", path = "plugin_sandbox_macos.rs")]
pub mod plugin_sandbox;
pub mod plugin_settings;
mod scheduler;
pub mod scheduler_v2;
pub mod settings;
pub use cap_negotiator::perform_cap_negotiation;

#[cfg(feature = "mpr_experimental")]
mod mpr;
#[cfg(feature = "mpr_experimental")]
pub use mpr::MprDispatcher;

// Multipath Data Plane (v1.0新機能)
pub mod multipath;

pub use ack::{build_ack_frame, parse_ack_frame, AckFrame, AckGenerator};
pub use builder::{build_header, build_header_ext};
pub use congestion::CongestionCtrl;
pub use frame::{
    parse_header, parse_header_ext, FrameHeader, FLAG_HAS_PATH_ID, FLAG_MULTIPATH_ENABLED,
};
pub use state::{Stream, StreamState};
pub use stream_frame::{build_stream_frame, parse_stream_frame, StreamFrame};
pub use tx::TxQueue;
pub mod layer;
pub use layer::StreamLayer;
mod reorder;
pub use reorder::ReorderBuffer;
mod receiver;
pub use receiver::MultipathReceiver;
mod sequencer;
pub use async_stream::{
    CleanupConfig, NyxAsyncStream, StreamError, StreamState as AsyncStreamState, StreamStats,
};
pub use error_handler::{
    ErrorCategory, ErrorContext, ErrorHandlerStats, ErrorSeverity, RecoveryAction,
    RecoveryStrategy, StreamErrorHandler,
};
pub use flow_controller::{
    CongestionState, FlowControlError, FlowControlStats, FlowController, RttEstimator,
};
pub use frame_handler::{
    FrameHandler, FrameHandlerError, FrameHandlerStats, FrameValidation, ReassembledData,
};
pub use integrated_frame_processor::{IntegratedFrameProcessor, StreamContext};
#[cfg(feature = "plugin")]
pub use plugin::PluginHeader;
pub use resource_manager::{
    ResourceError, ResourceInfo, ResourceLimits, ResourceManager, ResourceStats, ResourceType,
};
pub use sequencer::Sequencer;
// Export plugin frame utilities for both plugin and non-plugin builds.
#[cfg(feature = "plugin")]
pub use plugin_cbor::{
    parse_plugin_header, parse_plugin_header_bytes, serialize_plugin_header, PluginCborError,
    PluginHeader as CborPluginHeader, PluginId, MAX_CBOR_HEADER_SIZE, MAX_PLUGIN_DATA_SIZE,
};
#[cfg(feature = "plugin")]
pub use plugin_frame::build_plugin_frame;
pub use plugin_frame::{
    validate_plugin_frame_type, ParsedPluginFrame, PluginFrameError, PluginFrameProcessor,
    PluginFrameResult, PLUGIN_FRAME_TYPE_MAX, PLUGIN_FRAME_TYPE_MIN,
};
#[cfg(feature = "plugin")]
pub use plugin_geostat::{plugin_info, GeoStat, GEO_PLUGIN_ID};
#[cfg(feature = "plugin")]
pub use plugin_handshake::{HandshakeResult, PluginHandshakeCoordinator, PluginHandshakeError};
#[cfg(feature = "plugin")]
pub use plugin_manifest::ManifestItem as PluginManifestItem;
#[cfg(feature = "plugin")]
pub use plugin_registry::{Permission, PluginInfo, PluginRegistry};

#[cfg(not(feature = "plugin"))]
pub struct PluginHeader;
#[cfg(not(feature = "plugin"))]
pub struct PluginInfo;
#[cfg(not(feature = "plugin"))]
pub struct PluginRegistry;

pub use capability::{
    decode_caps, encode_caps, negotiate, Capability, NegotiationError, FLAG_REQUIRED, LOCAL_CAP_IDS,
};

pub use management::{
    build_close_frame, build_close_unsupported_cap, build_path_challenge_frame,
    build_path_response_frame, build_ping_frame, build_pong_frame, build_settings_frame,
    parse_close_frame, parse_path_challenge_frame, parse_path_response_frame, parse_ping_frame,
    parse_pong_frame, parse_settings_frame, CloseFrame, PathChallengeFrame, PathResponseFrame,
    PingFrame, PongFrame, Setting, SettingsFrame,
};

pub use management::setting_ids as management_setting_ids;
pub use settings::{settings_watch, StreamSettings};

pub use localized::{
    build_localized_string_frame, parse_localized_string_frame, LocalizedStringFrame,
};
pub use scheduler::WeightedRrScheduler;
pub use scheduler_v2::{PathInfo, SchedulerStats, WeightedRoundRobinScheduler};

#[cfg(feature = "hpke")]
pub mod hpke_rekey;
#[cfg(feature = "hpke")]
pub use hpke_rekey::{
    build_rekey_frame, open_rekey, parse_rekey_frame, seal_for_rekey, RekeyFrame,
};
#[cfg(feature = "hpke")]
pub mod hpke_rekey_manager;
#[cfg(feature = "hpke")]
pub use hpke_rekey_manager::{HpkeRekeyManager, RekeyDecision, RekeyPolicy};

#[cfg(test)]
mod tests;
