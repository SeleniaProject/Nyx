# nyx-core

Lightweight core utilities for the Nyx stack.

- Pure Rust, minimal dependencies
- Typed IDs and time helpers
- Robust config with env/file builder and validation
- i18n helpers (fluent-based), performance tools (EWMA, rate limiter)
- Low power utilities, path monitor, multipath scheduler
- Push abstraction and gateway (no TLS deps)
- Zero-copy utilities and plugin framework skeleton

## Quick start

```bash
# tests
cargo test -p nyx-core

# clippy
cargo clippy -p nyx-core --all-targets -- -D warnings

# example
cargo run -p nyx-core --example using_core

# benches
cargo bench -p nyx-core --bench zero_copy_benchmarks
```

## OS-level sandbox (optional)

When built with the `os_sandbox` feature, `nyx-core` can apply a minimal OS-enforced sandbox on supported platforms.

- Windows: a Job Object is created and the current process is assigned. The job is configured with Kill-on-job-close to ensure robust cleanup of child processes.
- Other platforms: currently unsupported (no-op).

API:

- `nyx_core::sandbox::apply_policy(SandboxPolicy::Minimal)` returns `SandboxStatus::{Applied, Unsupported}`.

Notes:

- This crate forbids `unsafe_code`; the implementation uses safe wrappers from `win32job`.
- Additional stricter policies may be added in the future under `SandboxPolicy::Strict`.
