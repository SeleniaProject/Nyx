use std::io::Write;

pub mod config;
pub mod error;
pub mod types;
#[cfg(target_os = "linux")]
pub mod sandbox;
pub mod i18n;
pub mod mobile;
pub mod push;
pub mod capability;
pub mod compliance;
pub mod cmix;
pub mod low_power;
pub mod advanced_routing;
pub mod performance;
pub mod path_monitor; // 新しい共有パス性能モニタ
// New v1.0 Critical Priority modules
#[cfg(feature = "plugin_framework")]
pub mod plugin_framework;
#[cfg(feature = "multipath_dp")]
pub mod multipath_dataplane;
#[cfg(feature = "cmix")]
pub mod cmix_integration;

pub use config::NyxConfig;
pub use config::PushProvider;
pub use config::MultipathConfig;
pub use error::NyxError;
pub use error::NyxResult;
pub use types::NodeId;
pub use types::PathId;
pub use path_monitor::{PathPerformanceMonitor, PathPerformanceMetrics, PerformanceTrend as PathPerformanceTrend, GlobalPathStats as CoreGlobalPathStats};
#[cfg(target_os = "linux")]
pub use sandbox::install_seccomp;
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
pub use compliance::{ComplianceLevel};

// Export new v1.0 Critical Priority components
#[cfg(feature = "plugin_framework")]
pub use plugin_framework::{PluginFramework, PluginHeader, PluginCapabilities};
#[cfg(feature = "multipath_dp")]
pub use multipath_dataplane::{MultipathDataPlane, PathId as MultipathPathId, PathMetrics};
#[cfg(feature = "cmix")]
pub use cmix_integration::{CMixIntegration, CMixConfig, BatchProcessor};
