# Nyx Stream: Plugin Framework Summary

## Overview

- Plugin frame range: 0x50–0x5F
- Header format: CBOR-serialized `PluginHeader { id, flags, data }`
- Dispatcher enforces registration and per-frame permissions

## Key modules

- plugin.rs: Core types, IDs, frame constants
- plugin_cbor.rs: Header decode helpers (+ tests)
- plugin_frame.rs: Full wire frame (encode/decode)
- plugin_registry.rs: Registry + permissions
- plugin_dispatch.rs: Safe dispatch, stats, logging hygiene (+ tests)
- plugin_handshake.rs: Minimal handshake payload helpers
- plugin_manifest.rs: TOML manifest + schema validation
- plugin_settings.rs: Runtime settings defaults
- plugin_ipc.rs: Pure-Rust in-proc IPC reference (bounded + backpressure)
- plugin_sandbox.rs: Cooperative sandbox guard (network/fs allow/deny + allowlists)

## Sandbox Implementation

The plugin framework provides two-layer security:

### Application-Level Guards (Always Active)
- Network and filesystem access validation before plugin operations
- Host allowlists and path prefix restrictions
- Cooperative enforcement for plugins using Nyx API

### OS-Level Process Sandboxing (Feature-Gated)
Enable with `features = ["os_sandbox"]` in your Cargo.toml:

- **Windows**: Job Objects with process limits and cleanup enforcement
- **Linux**: seccomp-bpf system call filtering (pure Rust implementation)
- **macOS**: sandbox_init with custom security profiles
- **OpenBSD**: pledge/unveil capability-based security
- **Other platforms**: Application-level guards only

#### Sandbox Policies
- `SandboxPolicy::Minimal`: Block dangerous operations (process creation, etc.)
- `SandboxPolicy::Strict`: Comprehensive restrictions including network/filesystem

## Run tests

PowerShell (Windows):

```powershell
# Basic tests
cargo test -p nyx-stream

# With OS-level sandbox
cargo test -p nyx-stream --features "os_sandbox"

# Cross-platform sandbox integration tests  
cargo test -p nyx-stream plugin_sandbox_integration --features "os_sandbox" -- --nocapture
```

## Notes

- All code in this crate avoids `unsafe`.
- Sandbox is cooperative (preflight checks in dispatcher) for application-level guards.
- OS-level sandbox provides kernel-enforced isolation when enabled via feature flag.
- Platform-specific implementations are automatically selected at compile time.
