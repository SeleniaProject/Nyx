# Windows Sandboxing (NyxNet)

This document describes the current Windows sandboxing implementation for NyxNet, complementing Linux seccomp and OpenBSD pledge/unveil described elsewhere.

## Status
- Core process isolation via Windows Job Objects is implemented in `nyx-core` and available behind a safe wrapper API.
- Additional restrictions (restricted token launch path for plugins, granular UI/job limits, policy from config) are planned and will extend the current foundation.

## Goals
- Constrain daemon and plugin processes to least privilege
- Limit resource abuse (CPU time, memory, handle count, process spawning)
- Provide defense‑in‑depth against plugin compromise

## Implemented Mechanism: Job Object Confinement
- Safe wrapper location: `nyx-core/src/windows.rs`
- Public API: `nyx_core::apply_process_isolation(Option<WindowsIsolationConfig>) -> io::Result<()>`
- Configuration type: `WindowsIsolationConfig` (fields shown below)

```rust
/// Configuration for Windows process isolation using Job Objects.
pub struct WindowsIsolationConfig {
    /// Per-process memory limit in megabytes (working set bound)
    pub max_process_memory_mb: usize,
    /// Total job memory limit in megabytes (reserved for future use)
    pub max_job_memory_mb: usize,
    /// Working set size limit in megabytes (applied via Job ExtendedLimitInfo)
    pub max_working_set_mb: usize,
    /// Maximum allowed CPU time per process in seconds (0 = unlimited)
    pub max_process_time_seconds: u64,
    /// Kill all associated processes when the job handle is closed
    pub kill_on_job_close: bool,
}
```

### Behavior
- A Job Object is created and configured using `ExtendedLimitInfo`.
- Working set bound is applied; when `kill_on_job_close` is set, all associated processes are terminated when the job is torn down.
- The current process is assigned to the job (child processes inherit restrictions).
- The job handle is intentionally kept alive for the process lifetime.

### Usage
```rust
use nyx_core::{apply_process_isolation, windows::WindowsIsolationConfig};

fn main() -> std::io::Result<()> {
    // Apply default, safe limits
    apply_process_isolation(None)?;

    // Or apply custom bounds
    let cfg = WindowsIsolationConfig { max_working_set_mb: 256, ..Default::default() };
    apply_process_isolation(Some(cfg))?;
    Ok(())
}
```

## Planned Extensions
1. Restricted token
   - Launch child processes with a restricted token (reduced SIDs/integrity level)
   - Deny writes to sensitive registry hives and system directories via integrity level + ACL
2. UI and handle limits
   - `JOB_OBJECT_UILIMIT_*` and handle count constraints where applicable
3. Filesystem sandbox root
   - Per‑instance data directory only; no write outside (enforced by ACL + code path discipline)
4. IPC filtering
   - Named pipes with `\\.\pipe\nyx_` prefix and broker pattern for privileged operations
5. Telemetry integration
   - Export limit hits and termination events for observability

## Fallback Behavior
- Non‑Windows builds: the module is gated by `cfg(target_os = "windows")` and is not compiled.
- Windows: if isolation is not applied explicitly, processes run without Job Object restrictions.

## Security Notes
- Parity with seccomp is not feasible on Windows; focus is on job/token limits and ACLs.
- Sensitive material continues to rely on memory zeroization primitives in the cryptographic layers.
