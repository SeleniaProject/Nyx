# Windows Sandboxing Plan (NyxNet)

This document outlines the planned Windows sandboxing approach for NyxNet components (not yet fully implemented in code). It complements Linux seccomp / OpenBSD pledge/unveil style restrictions described in the broader security design.

## Goals
- Constrain daemon / plugin processes to least privilege
- Limit resource abuse (CPU time, memory, handle count, process spawning)
- Provide defense‑in‑depth against plugin compromise

## Mechanisms
1. Job Object confinement
   - `CreateJobObject` + `AssignProcessToJobObject`
   - Limits: `JOB_OBJECT_LIMIT_PROCESS_TIME`, `JOB_OBJECT_LIMIT_PROCESS_MEMORY`, `JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION`, `JOB_OBJECT_LIMIT_ACTIVE_PROCESS`
   - UI restrictions: `JOB_OBJECT_UILIMIT_DESKTOP`, `JOB_OBJECT_UILIMIT_DISPLAYSETTINGS`
2. Restricted token
   - `CreateRestrictedToken` removing admin / high integrity SIDs
   - Deny write to sensitive registry hives & system dirs (via integrity level + ACL)
3. Filesystem sandbox root
   - Per‑instance data directory only; no write outside (enforced by ACL + code path discipline)
4. Inter‑process communication filtering
   - Named pipes prefixed with `\\.\pipe\nyx_` and randomized suffix
   - Broker pattern for plugin access to network / filesystem capabilities
5. Memory scrubbing & key material isolation
   - Continue to leverage `zeroize` on secret types

## Implementation Phases
| Phase | Status | Description |
|-------|--------|-------------|
| 1 | Planned | Introduce `sandbox::windows` module providing safe wrapper API (behind `windows_sandbox` feature) |
| 2 | Planned | Job object creation + assignment for daemon child plugin processes |
| 3 | Planned | Restricted token launch path for plugins |
| 4 | Planned | Resource limit configuration from `nyx.toml` (`[security]` section) |
| 5 | Planned | Telemetry integration: export sandbox limit hits / terminations |

## Minimal API (Planned)
```rust
pub struct WindowsSandboxConfig {
    pub max_processes: u32,
    pub memory_limit_bytes: u64,
    pub cpu_time_ms: Option<u64>,
}

pub fn launch_sandboxed(cmd: &str, args: &[&str], cfg: &WindowsSandboxConfig) -> anyhow::Result<ChildHandle> { /* windows only */ }
```

## Fallback Behavior
- Non‑Windows builds: stub module returns `Unsupported` error.
- Windows without feature enabled: processes launch normally (opt‑in safety).

## Security Notes
- Exact syscall filtering parity with seccomp is not practical on Windows; focus shifts to handle / token / job limits.
- Audit logging: on sandbox violation, emit telemetry event `sandbox.violation` with process id, rule, action.

## Next Steps
1. Add feature flag + stub module
2. Integrate into plugin launch path
3. Add tests (Windows only) verifying enforced memory/process limits using synthetic plugin
4. Document configuration schema in main docs

---
(Generated: 2025-08-10)
