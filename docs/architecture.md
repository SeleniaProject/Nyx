# Architecture Overview

NyxNet is organized as a Cargo workspace. Primary crates (per `Cargo.toml` members):

- nyx-daemon: control daemon (newline-delimited JSON RPC, auth, hot-reload, event streaming, Prometheus optional)
- nyx-cli: CLI to interact with the daemon
- nyx-core: common utilities (IDs, time, config builder, i18n, low-power tools, rate limiter, path monitor, multipath scheduler); minimal Windows OS sandbox via Job Object (feature-gated)
- nyx-crypto: cryptographic primitives (AEAD/KDF/HPKE), no unsafe
- nyx-transport: minimal UDP/TCP helpers
- nyx-stream: stream layer and plugin framework (CBOR header, registry, permissions, dispatch, minimal handshake)
- nyx-fec: FEC designed around 1280-byte shards with adaptive redundancy tuning
- nyx-conformance: deterministic network simulator and property-testing helpers
- nyx-sdk: application SDK (NyxStream, DaemonClient, etc.)
- nyx-sdk-wasm: SDK for browser/Node/WASI (Noise demo, push registration, multipath, HPKE)
- nyx-mobile-ffi: C-ABI for mobile platforms

## Process & IPC

- Daemon IPC uses newline-delimited JSON (one JSON per line).
- Endpoints: Unix `/tmp/nyx.sock`, Windows `\\.\\pipe\\nyx-daemon`.
- Operations: `get_info`, `reload_config`, `update_config`, `list_config_versions`, `rollback_config`, `create_config_snapshot`, `subscribe_events`.
- Token discovery: `NYX_DAEMON_TOKEN` → `NYX_DAEMON_COOKIE` or default cookie path → auto-generated if missing. Set `NYX_DAEMON_STRICT_AUTH=1` to enforce auth for privileged ops.

## Metrics

- Set `NYX_PROMETHEUS_ADDR` to expose `/metrics` from an embedded HTTP server.

## Dependency Policy

- Prefer pure Rust, avoid unsafe, and avoid external C/C++ backends. gRPC is intentionally disabled; JSON-RPC is used instead.
