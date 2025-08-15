use std::io::Write;

pub mod advanced_routing;
pub mod capability;
pub mod cmix;
pub mod compliance;
pub mod config;
pub mod error;
#[cfg(feature = "mobile_ffi")]
mod ffi_detector; // internal module providing FfiScreenStateDetector
pub mod i18n;
pub mod low_power;
pub mod mobile;
pub mod path_monitor;
pub mod performance;
pub mod push;
pub mod push_gateway; // Push Notification Path / Gateway reconnection
#[cfg(target_os = "linux")]
pub mod sandbox;
pub mod types;
#[cfg(target_os = "windows")]
pub mod windows;
pub mod zero_copy; // Zero-copy optimization for critical data paths // 新しい共有パス性能モニタ
                   // New v1.0 Critical Priority modules
#[cfg(feature = "cmix")]
pub mod cmix_integration;
#[cfg(feature = "multipath_dp")]
pub mod multipath_dataplane;
#[cfg(feature = "plugin_framework")]
pub mod plugin_framework;

pub use config::MultipathConfig;
pub use config::NyxConfig;
pub use config::PushProvider;
pub use error::NyxError;
pub use error::NyxResult;
pub use path_monitor::{
    GlobalPathStats as CoreGlobalPathStats, PathPerformanceMetrics, PathPerformanceMonitor,
    PerformanceTrend as PathPerformanceTrend,
};
#[cfg(target_os = "linux")]
pub use sandbox::install_seccomp;
pub use types::NodeId;
pub use types::PathId;
#[cfg(target_os = "windows")]
pub use windows::apply_process_isolation;
#[cfg(target_os = "openbsd")]
pub mod openbsd;
#[cfg(target_os = "openbsd")]
pub use openbsd::{install_pledge, unveil_path};

/// Install a panic hook that ensures `abort` so systemd captures core dump.
pub fn install_panic_abort() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("panic: {info}");
        // Flush stderr then abort using safe std::process::abort
        std::io::stderr().flush().ok();
        std::process::abort();
    }));
}

pub use capability::{Capability, FLAG_REQUIRED};
pub use compliance::ComplianceLevel;
#[cfg(feature = "mobile_ffi")]
pub use ffi_detector::FfiScreenStateDetector;

// Export new v1.0 Critical Priority components
#[cfg(feature = "cmix")]
pub use cmix_integration::{BatchProcessor, CMixConfig, CMixIntegration};
#[cfg(feature = "multipath_dp")]
pub use multipath_dataplane::{MultipathDataPlane, PathId as MultipathPathId, PathMetrics};
#[cfg(feature = "plugin_framework")]
pub use plugin_framework::{PluginCapabilities, PluginFramework, PluginHeader};
