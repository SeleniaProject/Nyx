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
