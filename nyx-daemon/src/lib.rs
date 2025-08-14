// Library entry for nyx-daemon exposing path builder & supporting modules for other crates
pub mod proto; // always available
pub mod capability; // capability management
pub mod push; // push notification mock
// Metrics collector: real module when feature enabled, otherwise provide a minimal stub for tests/integration.
#[cfg(feature = "experimental-metrics")]
pub mod metrics;
#[cfg(not(feature = "experimental-metrics"))]
pub mod metrics {
    #[derive(Clone, Default)]
    pub struct MetricsCollector;
    impl MetricsCollector { pub fn new() -> Self { Self } }
}
pub mod layer_manager; // expose layer manager for integration/tests
#[cfg(feature = "experimental-metrics")] pub mod zero_copy_bridge; // export zero-copy metrics bridge for tests/integration
#[cfg(feature = "experimental-metrics")] pub mod prometheus_exporter; // export prometheus exporter for integration tests
#[cfg(feature = "path-builder")] pub mod path_builder_broken; // heavy module guarded by feature
#[cfg(feature = "path-builder")] pub use path_builder_broken as path_builder; // maintain expected name
pub mod pure_rust_dht; // always expose minimal DHT for integration
#[cfg(feature = "experimental-alerts")] pub mod alert_system; // alert system

// Helper exported items
#[cfg(feature = "path-builder")] pub use path_builder_broken::{PathBuilder, PathBuilderConfig};

// Utility helpers duplicated from main for library context
use std::time::SystemTime;
use once_cell::sync::Lazy;
use nyx_core::path_monitor::PathPerformanceRegistry;

// グローバル共有 PathPerformanceRegistry (ライブラリ/バイナリ双方から利用)
pub static GLOBAL_PATH_PERFORMANCE_REGISTRY: Lazy<PathPerformanceRegistry> = Lazy::new(|| PathPerformanceRegistry::new());

pub fn system_time_to_proto_timestamp(time: SystemTime) -> proto::Timestamp {
	let duration = time
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap_or_default();
	proto::Timestamp { seconds: duration.as_secs() as i64, nanos: duration.subsec_nanos() as i32 }
}
