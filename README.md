<div align="center">

# NyxNet

Modular, privacy-first networking stack in Rust. Clean architecture, safe-by-default, and OSS-friendly.

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE-APACHE)
![Rust Edition](https://img.shields.io/badge/Rust-2021-orange)
![OS](https://img.shields.io/badge/OS-Linux%20%7C%20macOS%20%7C%20Windows-555)

</div>

## Overview

NyxNet is a multi-crate Rust workspace that brings together a daemon, CLI, SDKs, and transport/stream/crypto utilities to build privacy-preserving networked apps. Dual-licensed under MIT or Apache-2.0.

## Highlights

- nyx-daemon: pure Rust daemon with newline-delimited JSON RPC over UDS/Named Pipe, hot-reloadable config, event streaming, optional Prometheus.
- nyx-cli: CLI for daemon operations and quick smoke tests.
- nyx-core: IDs, time, config builder, i18n, rate limiting, Windows minimal sandbox feature.
- nyx-crypto: pure Rust AEAD/KDF/HPKE building blocks (no unsafe).
- nyx-transport: minimal UDP/TCP helpers.
- nyx-stream: stream layer with plugin framework (permissions, CBOR headers).
- nyx-fec: FEC for fixed 1280-byte shards with adaptive redundancy tuning.
- nyx-conformance: deterministic network simulator and property testing helpers.
- nyx-sdk: app-facing SDK (async stream, daemon client).
- nyx-sdk-wasm: WASM SDK for browser/Node/WASI (Noise demo, push, multipath, HPKE).
- nyx-mobile-ffi: C-ABI for mobile integration.

See `Cargo.toml` workspace members for the authoritative list.

## Quick start (local)

1) Build

```
cargo build --release
```

2) Explore binaries

```
./target/release/nyx-daemon --help
./target/release/nyx-cli --help
```

3) Configure

- Primary config file: `nyx.toml`
- Minimal example:

```toml
# nyx.toml (excerpt)
log_level = "info"
listen_port = 43300

[network]
bind_addr = "127.0.0.1:43300"

# If present, applied as the default Nyx stream safety cap at daemon boot
max_frame_len_bytes = 8_388_608
```

The daemon logs at info level by default. Override via `RUST_LOG`.

## IPC / API

- Newline-delimited JSON request/response.
- Endpoints: Unix `/tmp/nyx.sock`, Windows `\\.\\pipe\\nyx-daemon`.
- Core ops: `get_info`, `reload_config` (auth), `update_config` (auth), `list_config_versions` (auth), `rollback_config` (auth), `create_config_snapshot` (auth), `subscribe_events` (auth; switches to stream mode).
- Token discovery order: `NYX_DAEMON_TOKEN` → `NYX_DAEMON_COOKIE` (or default cookie path) → auto-generated at boot if missing. Set `NYX_DAEMON_STRICT_AUTH=1` to require a valid token for privileged ops.

See `docs/api.md` for details.

## Metrics

- Optional Prometheus exporter. Set `NYX_PROMETHEUS_ADDR` (e.g. `127.0.0.1:0`) to expose `/metrics` via an embedded HTTP server.

## Kubernetes / Helm

- Charts live in `charts/nyx`. Refer to `docs/quickstart-ubuntu-k8s.md` for a minimal walkthrough.

## Documentation

- Start here: `docs/index.md`
- Architecture: `docs/architecture.md`
- Configuration: `docs/configuration.md`
- IPC/API: `docs/api.md`
 - Specifications overview: `docs/specs.md`

## Specifications (brief)

- Nyx Protocol v1.0 (draft; includes planned features): `spec/Nyx_Protocol_v1.0_Spec_EN.md`
	- Multipath data plane (per-packet PathID), extended header with 12-byte CID, fixed 1280B payloads
	- Hybrid post-quantum handshake (X25519 + Kyber) and HPKE support; anti-replay window 2^20 per direction
	- Plugin frames 0x50–0x5F with CBOR header; capability negotiation via CBOR list (see policy below)
	- Optional cMix mode (batch≈100, VDF≈100ms), adaptive cover traffic (target utilization 0.2–0.6)
	- Compliance levels: Core / Plus / Full; telemetry: OTLP spans alongside Prometheus
	- Note: Some items are roadmap-level and may not be fully implemented yet.
- Nyx Protocol v0.1 (baseline implemented set): `spec/Nyx_Protocol_v0.1_Spec_EN.md`
- Capability Negotiation Policy: `spec/Capability_Negotiation_Policy_EN.md`
	- CBOR entries `{id:u32, flags:u8, data:bytes}`; flags bit 0x01 = Required
	- Unsupported Required capability → CLOSE 0x07 with the 4-byte unsupported ID in reason
- Design Document: `spec/Nyx_Design_Document_EN.md`
	- Principles: security-by-design, performance without compromise, modularity, formal methods
	- Layers: secure stream, mix routing, obfuscation+FEC, transport; async pipeline with backpressure
	- Cryptography: AEAD/KDF/HPKE, key rotation, PQ readiness; threat model covers global passive/active

## Contributing

We welcome contributions! Please review `CONTRIBUTING.md` and `CODE_OF_CONDUCT.md`. Keep changes safe, focused, and well-tested.

## License

Licensed under either of:

- MIT (`LICENSE-MIT`)
- Apache-2.0 (`LICENSE-APACHE`)

Choose the license that best fits your needs.

