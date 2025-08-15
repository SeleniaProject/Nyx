#![forbid(unsafe_code)]

use std::sync::Arc;
use std::time::Duration;

use nyx_core::zero_copy::manager::ZeroCopyManager;

/// Start a background task that periodically exports Zero-Copy manager metrics
/// into the global Prometheus recorder via the `metrics` crate.
pub fn start_zero_copy_metrics_task(manager: Arc<ZeroCopyManager>) {
    start_zero_copy_metrics_task_with_interval(manager, Duration::from_secs(10));
}

/// Same as above but with custom interval (for tests).
pub fn start_zero_copy_metrics_task_with_interval(manager: Arc<ZeroCopyManager>, every: Duration) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(every);
        loop {
            interval.tick().await;
            let aggregated = manager.get_aggregated_metrics().await;

            // Export as counters/gauges; counters use absolute to avoid double-add.
            metrics::counter!("nyx_zero_copy_combined_allocations")
                .absolute(aggregated.combined_allocations);
            metrics::counter!("nyx_zero_copy_combined_bytes").absolute(aggregated.combined_bytes);
            metrics::counter!("nyx_zero_copy_allocation_overhead_ns")
                .absolute(aggregated.total_allocation_overhead_ns as u64);

            metrics::gauge!("nyx_zero_copy_total_paths").set(aggregated.total_paths as f64);
            metrics::gauge!("nyx_zero_copy_average_zero_copy_ratio")
                .set(aggregated.average_zero_copy_ratio);
            metrics::gauge!("nyx_zero_copy_average_reduction_ratio")
                .set(aggregated.average_reduction_ratio);
        }
    });
}
