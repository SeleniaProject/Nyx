#![cfg(target_os = "windows")]
#![forbid(unsafe_code)]

//! Windows process isolation utilities for Nyx core.
//!
//! This module applies Job Object based process restrictions to the current
//! process to achieve resource and privilege containment on Windows.
//! It mirrors the seccomp/pledge integration on other platforms.

use std::io;
use std::time::Duration;
use tracing::{debug, info, warn};
use win32job::{Job, ExtendedLimitInfo, UiRestrictions};

/// Configuration for Windows process isolation using Job Objects.
#[derive(Debug, Clone)]
pub struct WindowsIsolationConfig {
    /// Per-process memory limit in megabytes.
    pub max_process_memory_mb: usize,
    /// Total job memory limit in megabytes.
    pub max_job_memory_mb: usize,
    /// Working set size limit in megabytes.
    pub max_working_set_mb: usize,
    /// Maximum allowed CPU time per process in seconds.
    pub max_process_time_seconds: u64,
    /// Apply UI restrictions (desktop/exit/windows station limits).
    pub ui_restrictions_enabled: bool,
    /// Kill all associated processes when the job handle is closed.
    pub kill_on_job_close: bool,
}

impl Default for WindowsIsolationConfig {
    fn default() -> Self {
        Self {
            max_process_memory_mb: 512,
            max_job_memory_mb: 1024,
            max_working_set_mb: 256,
            max_process_time_seconds: 0, // 0 means no explicit CPU time limit
            ui_restrictions_enabled: true,
            kill_on_job_close: true,
        }
    }
}

/// Apply Windows Job Object based isolation to the current process.
///
/// This uses only safe Rust wrappers; no unsafe blocks are required.
pub fn apply_process_isolation(config: Option<WindowsIsolationConfig>) -> io::Result<()> {
    let cfg = config.unwrap_or_default();
    info!(
        target: "nyx-core::windows",
        "Applying Windows process isolation: {:?}", cfg
    );

    // Create a Job Object using the safe wrapper
    let job = Job::create()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to create Job Object: {}", e)))?;

    // Configure extended limits
    let mut limit_info = ExtendedLimitInfo::new();

    // Set memory limits
    let process_bytes = cfg.max_process_memory_mb.saturating_mul(1024 * 1024);
    let job_bytes = cfg.max_job_memory_mb.saturating_mul(1024 * 1024);
    let working_bytes = cfg.max_working_set_mb.saturating_mul(1024 * 1024);

    limit_info.limit_process_memory(process_bytes, process_bytes);
    limit_info.limit_job_memory(job_bytes);
    limit_info.limit_working_memory(working_bytes, working_bytes);

    // Set process CPU time limit if requested
    if cfg.max_process_time_seconds > 0 {
        limit_info.limit_process_time(Duration::from_secs(cfg.max_process_time_seconds));
    }

    if cfg.kill_on_job_close {
        limit_info.limit_kill_on_job_close();
    }

    job
        .set_extended_limit_info(&mut limit_info)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to set job limits: {}", e)))?;

    // Optional UI restrictions
    if cfg.ui_restrictions_enabled {
        let mut ui = UiRestrictions::new();
        ui.limit_desktop();
        ui.limit_display_settings();
        ui.limit_exit_windows();
        if let Err(e) = job.set_ui_restrictions(&ui) {
            warn!(target: "nyx-core::windows", "Failed to set UI restrictions: {}", e);
        }
    }

    // Assign current process to the job
    let pid = std::process::id();
    job.assign_process(pid)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to assign current process to job: {}", e)))?;

    debug!(target: "nyx-core::windows", "Job Object configured and assigned. pid={}", pid);

    // Leak the job handle to keep restrictions alive for the remainder of the process lifetime.
    // Windows releases the job when the process terminates; this ensures child processes inherit limits.
    std::mem::forget(job);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let cfg = WindowsIsolationConfig::default();
        assert!(cfg.max_process_memory_mb >= 128);
        assert!(cfg.max_job_memory_mb >= cfg.max_process_memory_mb);
        assert!(cfg.max_working_set_mb > 0);
        assert!(cfg.kill_on_job_close);
    }
}


