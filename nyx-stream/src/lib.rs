#![forbid(unsafe_code)]

pub mod advanced_path_selection; // Advanced path selection algorithms and load balancing
pub mod advanced_rate_limiting; // Advanced Rate Limiting & Flow Control for v1.0
pub mod async_stream;
pub mod builder;
pub mod capability; // Capability negotiation
pub mod cmix_integration; // cMix Integration (VDF-based Batch Processing) for v1.0
pub mod comprehensive_error_handling; // Comprehensive error handling for v1.0
pub mod congestion;
pub mod dynamic_latency_selection; // Dynamic latency-based path selection for v1.0
pub mod early_data; // Early-Data and 0-RTT Reception for v1.0
pub mod errors;
pub mod extended_packet; // Extended packet format for v1.0
pub mod flow_controller;
pub mod frame;
pub mod frame_codec;
pub mod hpke_rekey;
pub mod integrated_frame_processor;
pub mod management; // Management frame_s and error code_s
pub mod multipath;
pub mod multipath_dataplane; // LARMix++ Dynamic Path Selection for v1.0
pub mod padding_system; // Comprehensive Padding & Traffic Analysis Resistance for v1.0
pub mod performance; // Performance optimization module
pub mod plugin; // Plugin core type_s
pub mod plugin_cbor; // CBOR header parsing helpers
pub mod plugin_dispatch; // Dispatcher for plugin frame_s
pub mod plugin_frame; // Full plugin frame (CBOR-serializable)
pub mod plugin_framework; // Protocol Combinator (Plugin Framework) for v1.0
pub mod plugin_handshake; // Handshake helpers and type_s
pub mod plugin_ipc; // IPC helper trait_s (stub_s for now)
pub mod plugin_manifest; // Manifest loader (TOML)
pub mod plugin_registry; // In-memory registry and permission_s
pub mod plugin_sandbox; // Cooperative sandbox (policy + guard_s)
pub mod plugin_sandbox_platform; // Platform-specific sandbox implementations
pub mod plugin_settings;
pub mod telemetry_schema;
pub mod test_helpers; // Test helper utilities for integration tests // OTLP Telemetry Schema for Nyx Protocol v1.0 // Runtime setting_s for plugin_s // Rekey trigger helpers (test_s/integration use) // Integrated frame processing with reordering and flow control

pub use advanced_path_selection::{
    AdvancedPathSelectionConfig, AdvancedPathSelector, BandwidthStatistics, CongestionMetrics,
    FailoverConfig, LoadBalancingConfig, LossStatistics, PathSelectionAlgorithm, PathStatistics,
    RttStatistics, SelectionMetrics,
};
pub use advanced_rate_limiting::{
    AdvancedFlowConfig, AdvancedFlowController, BackpressureCause, BackpressureController,
    BackpressureEvent, BucketStatus, FlowControlStatus, NyxRateLimiter, PriorityTokenBucket,
    RateLimitError, RateLimiterStats, RateLimiterStatus, TokenBucket, TrafficType,
    TransmissionDecision,
};
pub use cmix_integration::{
    BatchProcessingState, BatchState, CmixConfig, CmixFrame, CmixIntegrationError,
    CmixIntegrationManager, CmixStats,
};
pub use comprehensive_error_handling::{
    ErrorCategory, ErrorHandler, ErrorHandlingConfig, ErrorSeverity, ErrorStatistics, IntoNyxError,
    NyxError, RecoveryStrategy,
};
pub use dynamic_latency_selection::{
    DynamicLatencyConfig, DynamicLatencySelector, LatencyClassification, LatencyStats,
};
pub use multipath_dataplane::{
    AntiReplayWindow as MultipathAntiReplayWindow, ConnectionId as MultipathConnectionId,
    MultipathConfig, MultipathDataPlane, MultipathMetrics, PathId as MultipathPathId, PathInfo,
    PathMetrics, PathScheduler, PathState, ReorderingBuffer,
};
pub use padding_system::{
    FramePaddingProcessor, PaddingConfig, PaddingError, PaddingManager, TrafficMetrics,
    DEFAULT_TARGET_PACKET_SIZE, MAX_TIMING_DELAY, MIN_PADDING_SIZE,
};
pub use plugin_framework::{
    CapabilityNegotiator, Plugin, PluginCapability, PluginError, PluginFrameType, PluginHeader,
    PluginManager, PluginManagerConfig, PluginMetadata, PluginState,
};

pub use async_stream::{pair, AsyncStream, AsyncStreamConfig};
pub use capability::{get_local_capabilities, negotiate, Capability, CapabilityError};
pub use early_data::{
    AntiReplayStats, AntiReplayWindow, DirectionId, EarlyDataManager, EarlyDataMetrics,
    EarlyDataState, Nonce, NonceConstructor, SessionStats, ANTI_REPLAY_WINDOW_SIZE,
    MAX_EARLY_DATA_SIZE, MAX_TOTAL_EARLY_DATA,
};
pub use errors::{Error, Result};
pub use extended_packet::{
    ConnectionId, ExtendedPacket, ExtendedPacketBuilder, ExtendedPacketHeader, PacketFlags,
    PacketType, PathId,
};
pub use frame::{Frame, FrameHeader, FrameType};
pub use frame_codec::FrameCodec;
pub use integrated_frame_processor::{IntegratedFrameProcessor, ProcessorConfig};
pub use management::{build_close_unsupported_cap, parse_close_unsupported_cap};
pub use telemetry_schema::{
    attribute_names, span_names, ConnectionId as TelemetryConnectionId,
    NyxTelemetryInstrumentation, StreamTelemetryContext, TelemetryConfig, TelemetrySampler,
};

// Test modules
#[cfg(test)]
pub mod tests {
    pub mod frame_handler_tests;
    pub mod integrated_frame_processor_tests;
}
