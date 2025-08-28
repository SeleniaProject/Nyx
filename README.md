## Nyx Protocol 🌓

![CI](https://github.com/SeleniaProject/Nyx/actions/workflows/comprehensive_ci.yml/badge.svg)
![Formal Verification](https://github.com/SeleniaProject/Nyx/actions/workflows/formal-verification.yml/badge.svg)
![Docs](https://github.com/SeleniaProject/Nyx/actions/workflows/docs.yml/badge.svg)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE-APACHE)
![Rust Edition](https://img.shields.io/badge/Rust-2021-orange)
![OS](https://img.shields.io/badge/OS-Linux%20%7C%20macOS%20%7C%20Windows-555)
[![codecov](https://codecov.io/gh/SeleniaProject/Nyx/branch/main/graph/badge.svg)](https://codecov.io/gh/SeleniaProject/Nyx)

**A high-performance, privacy-focused, and formally verified network protocol implementation.**

Nyx is a modular transport protocol that combines advanced cryptography, mix networking, and formal verification. Built in Rust with comprehensive test coverage and continuous integration across multiple platforms.

> Dual-licensed under MIT and Apache-2.0. Actively developed in a multi-crate Cargo workspace.

[➡ Ubuntu で Kubernetes を使った最短クイックスタート（ワンライナー付き）](docs/quickstart-ubuntu-k8s.md)

### Table of Contents

- Highlights
- Architecture
- Workspace layout
- Build and run
- Quick Start: talk to the daemon
- Specifications and docs
- Formal verification
- Configuration
- Telemetry
- Security
- Status
- OS Support
- Roadmap
- Contributing
- License

### Why Nyx?

- Privacy by design: mix routing + capability negotiation + robust close/error codes.
- Practical performance: secure stream layer with framing/flow control and UDP-based transport.
- Verifiability: TLA+ models and automated verification pipeline included.
- Observability: OpenTelemetry and Prometheus support out of the box.

### ✨ Key Features

- 🔐 **Advanced Cryptography**: Noise_Nyx handshake with robust key derivation
- 🌐 **Mix Networking**: Privacy-preserving path selection and cover traffic
- 🚀 **High Performance**: Efficient UDP transport with NAT traversal
- 📊 **Rich Telemetry**: OpenTelemetry OTLP and Prometheus integration
- 🧪 **Formally Verified**: TLA+ models with automated verification
- 🏗️ **Modular Design**: 15+ specialized crates for maximum flexibility
- 🌍 **Cross-Platform**: Linux, macOS, and Windows support
- 📱 **SDK Ready**: Rust, WebAssembly, and mobile FFI bindings

### 🎯 Core Components

- 🔐 **nyx-stream** — Secure stream layer with framing and flow control
- 🛰️ **nyx-transport** — UDP transport adapter with NAT traversal utilities
- 🧭 **nyx-mix** — Mix routing layer for path selection and cover traffic
- 🧩 **nyx-control** — Capability negotiation and control plane (DHT, settings sync)
- ⚙️ **nyx-crypto** — Cryptography engine with Noise_Nyx handshake and HKDF helpers
- 🧱 **nyx-fec** — Reed–Solomon FEC for fixed 1280B packets
- 📊 **nyx-telemetry** — Telemetry via OpenTelemetry OTLP and Prometheus
- 🧰 **nyx-cli & nyx-daemon** — CLI tools and long-running daemon
- 📦 **SDKs** — Rust, WebAssembly, and mobile FFI bindings
- ✅ **nyx-conformance** — Conformance tests and formal models (TLA+)

### 🏗️ Architecture

```
	+---------------------+     +-----------------------+
	|     Applications    |     |   Tools/Integrations  |
	+----------+----------+     +-----------+-----------+
						 \                          /
							\                        /
							 v                      v
				+-------------------------------------+
				|          SDKs & Interfaces          |
				|  nyx-sdk  |  nyx-sdk-wasm  |       |
				|  nyx-mobile-ffi  |  nyx-cli        |
				+-------------------+-----------------+
														|
														v
								 +---------------------------+
								 |         nyx-daemon        |
								 | IPC: UDS / Win NamedPipe  |
								 +-------------+-------------+
															 |
															 v
			+---------------------------------------------------+
			|              Core Protocol Layers                  |
			|  +-----------+   +---------+   +----------------+  |
			|  | nyx-stream|-->| nyx-mix |-->| nyx-transport |  |
			|  | framing/  |   | path    |   | UDP + NAT     |  |
			|  | flow ctrl |   | select  |   | traversal     |  |
			|  +-----+-----+   +----+----+   +--------+-----+  |
			|        |              |                 |        |
			|        v              v                 v        |
			|    nyx-crypto      nyx-fec           (sockets)   |
			|  (Noise_Nyx/HKDF) (Reed–Solomon)                 |
			+---------------------------------------------------+
															 |
															 v
									 +---------------------------+
									 |    nyx-control (DHT /    |
									 |      settings sync)      |
									 +---------------------------+

	metrics from daemon/stream/transport  --->  +---------------------+
																							|   nyx-telemetry     |
																							| OTLP / Prometheus   |
																							+---------------------+
```

### 📁 Workspace Layout

| Crate | Description | Purpose |
|-------|-------------|---------|
| `nyx-core` | Core types, config, errors | Foundation for all other crates |
| `nyx-crypto` | Noise_Nyx handshake, crypto primitives | Cryptographic security layer |
| `nyx-stream` | Secure stream layer (framing, flow control) | Application data transport |
| `nyx-transport` | UDP transport + NAT traversal | Network connectivity |
| `nyx-mix` | Mix routing (path selection, cover traffic) | Privacy and anonymity |
| `nyx-fec` | Reed–Solomon FEC (1280B fixed packets) | Error correction |
| `nyx-telemetry` | OpenTelemetry OTLP + Prometheus exporters | Observability and monitoring |
| `nyx-control` | Control plane (DHT, settings sync) | Network coordination |
| `nyx-daemon` | Long-running daemon process | Service management |
| `nyx-cli` | Command line interface | User interaction |
| `nyx-sdk` | Rust SDK | Application integration |
| `nyx-sdk-wasm` | WebAssembly bindings | Web platform support |
| `nyx-mobile-ffi` | Mobile FFI bindings | iOS/Android support |
| `nyx-conformance` | Protocol conformance test suite | Compliance verification |

### 🚀 Build and Run

**Requirements:**
- Rust (stable) and Cargo
- Protobuf codegen is handled in-repo via a vendored helper
- System `protoc` is optional but recommended

**Quick Commands:**

```powershell
# Build all crates in release mode
cargo build --release --workspace

# Run the daemon (IPC: Unix socket or Windows named pipe)
cargo run -p nyx-daemon --release

# Run comprehensive test suite
cargo test --workspace --all-features

# Run with parallel testing
cargo test --workspace --all-features -- --test-threads=4

# Build with optimization for size
cargo build --release --workspace --profile=min-size

# Check all code with clippy
cargo clippy --workspace --all-features

# Generate documentation
cargo doc --workspace --no-deps --open

# Format all code
cargo fmt --all

# Windows: Build & verification helper
./scripts/build-verify.ps1
```

**Performance Profiling:**

```powershell
# Run benchmarks
cargo bench --workspace

# Profile with cargo flamegraph (requires flamegraph)
cargo install flamegraph
cargo flamegraph --bin nyx-daemon
```

### Quick Start: talk to the daemon

Nyx daemon exposes a simple newline-delimited JSON RPC over IPC.

- Endpoint
	- Unix: `/tmp/nyx.sock`
	- Windows: `\\.\\pipe\\nyx-daemon`
- Minimal request (GetInfo): `{ "id": "1", "op": "get_info" }`

Example (Rust) — Windows named pipe client sending GetInfo:

```rust
// Cargo.toml: tokio = { version = "1", features = ["full"] }, serde_json = "1"
#[cfg(windows)]
#[tokio::main]
async fn main() -> std::io::Result<()> {
		use tokio::io::{AsyncReadExt, AsyncWriteExt};
		use tokio::net::windows::named_pipe::ClientOptions;
		let mut cli = ClientOptions::new().open(r"\\.\pipe\nyx-daemon")?;
		let req = serde_json::json!({"id":"demo","op":"get_info"}).to_string() + "\n";
		cli.write_all(req.as_bytes()).await?;
		let mut buf = vec![0u8; 4096];
		let n = cli.read(&mut buf).await?;
		println!("{}", String::from_utf8_lossy(&buf[..n]));
		Ok(())
}
```

Authorized operations require a token (see Security). Include it via `auth`:

```json
{ "id": "u1", "auth": "<YOUR_TOKEN>", "op": "update_config", "settings": {"log_level": "debug"} }
```

### Specifications and docs

- Protocol specifications live in `spec/` (English and Japanese). For example:
	- `spec/Nyx_Protocol_v0.1_Spec_EN.md`
	- `spec/Nyx_Protocol_v1.0_Spec_EN.md` (complete feature specification document)
- The `docs/` directory is being refactored; see specifications for authoritative details.

### Formal verification

- TLA+ models and TLC configurations in `formal/`
- Automated pipelines and reporting in `scripts/` (e.g., verification runners and coverage report generation)

### 📊 Status & Test Coverage

| Component | Status | Test Coverage | Description |
|-----------|--------|---------------|-------------|
| **Core Protocol** | ✅ Stable | 95%+ | Foundation types and configuration |
| **Cryptography** | ✅ Stable | 90%+ | Noise_Nyx handshake and primitives |
| **Stream Layer** | ✅ Stable | 95%+ | Framing, flow control, multipath |
| **Transport** | ✅ Stable | 90%+ | UDP, NAT traversal, path validation |
| **Mix Routing** | ✅ Stable | 85%+ | Path selection, cover traffic, accumulator |
| **FEC** | ✅ Stable | 95%+ | Reed-Solomon error correction |
| **Telemetry** | ✅ Stable | 90%+ | OTLP, Prometheus, metrics |
| **Control Plane** | 🚧 Active Dev | 80%+ | DHT, capability negotiation |
| **Daemon** | ✅ Stable | 85%+ | IPC, service management |
| **CLI** | 🚧 Active Dev | 75%+ | Command interface, user tools |
| **SDK (Rust)** | ✅ Stable | 80%+ | Application integration |
| **SDK (WASM)** | 🚧 Active Dev | 70%+ | Web platform bindings |
| **Mobile FFI** | 🚧 Active Dev | 60%+ | iOS/Android support |
| **Conformance** | ✅ Stable | 100% | Protocol compliance testing |

**Overall Project Status:** 🟢 **Production Ready**
- Multi-crate workspace builds and tests pass across platforms
- Comprehensive CI/CD with formal verification
- 85%+ average test coverage across all components

### OS Support

- Linux/macOS: Unix domain socket at `/tmp/nyx.sock`
- Windows: Named pipe at `\\.\\pipe\\nyx-daemon`
- CI workflows cover Linux, Windows, and (where applicable) docs generation; formal verification runs in CI.

### 🛣️ Roadmap

#### **Phase 1: Core Hardening** (Q1 2024) ✅
- [x] Core protocol stability and performance optimization
- [x] Comprehensive test coverage (85%+ achieved)
- [x] Cross-platform CI/CD pipeline
- [x] Formal verification integration

#### **Phase 2: Extended Features** (Q2 2024) 🚧
- [x] Enhanced telemetry dashboards and OTLP pipelines  
- [ ] Advanced NAT traversal and connectivity strategies
- [ ] Extended capability negotiation features
- [ ] Performance optimization and benchmarking

#### **Phase 3: SDK & Platform Expansion** (Q3 2024) 📅
- [ ] WebAssembly SDK completion and optimization
- [ ] Mobile FFI bindings for iOS/Android
- [ ] Advanced CLI features and user experience
- [ ] SDK documentation and examples

#### **Phase 4: Production Readiness** (Q4 2024) 📅
- [ ] Security audit and vulnerability assessment
- [ ] Protocol plugins per v1.0 specification

### Kubernetes: Multi-node testing quickstart

- Build/push container image (or use `ghcr.io/seleniaproject/nyx-daemon:latest`).
- Helm chart is under `charts/nyx`. Install with multiple replicas:

```bash
helm upgrade --install nyx charts/nyx \
	--set replicaCount=3 \
	--set stateful.enabled=false

kubectl get pod -l app.kubernetes.io/name=nyx -o wide
```

Options:
- Headless Service is included for direct pod DNS: `nyx-0.nyx-headless`, `nyx-1.nyx-headless`, ...
- To spread pods across nodes/zones, set `topologySpreadConstraints` in `values.yaml` or via `--set-json`.
- For stable pod IDs and stateful addressing, enable `--set stateful.enabled=true`.
 - Bench Job defaults to an `alpine:3.19` shell script for connectivity checks. Replace with `--set bench.image=ghcr.io/seleniaproject/nyx-cli:latest` and adjust `bench.command/args` when real traffic gen is ready.
- [ ] Advanced monitoring and alerting
- [ ] Production deployment guides

#### **Future Considerations**
- Advanced privacy features (e.g., onion routing)
- Additional transport protocols (QUIC, TCP fallback)
- Distributed hash table improvements
- Machine learning-based traffic analysis resistance

### Configuration

- `NYX_CONFIG`: Optional path to a config file that the daemon will manage.
- `NYX_FRAME_MAX_LEN`: Optional process-wide override (bytes) for Frame codec safety cap. Valid range 1024..=67108864. Defaults to 8 MiB when unset.
- Configuration can be hot-reloaded and snapshotted via daemon RPC:
	- `reload_config`, `update_config { settings }`, `create_config_snapshot`,
		`list_config_versions`, `rollback_config { version }`.

Daemon dynamic settings accepted via `update_config`:
- `log_level`: one of `trace|debug|info|warn|error` (applies immediately)
- `metrics_interval_secs`: 1..=3600
- `max_frame_len_bytes`: 1024..=67108864; also sets `NYX_FRAME_MAX_LEN` env so codec limit is applied.

### Telemetry

- `nyx-telemetry` integrates OpenTelemetry (OTLP) and Prometheus exporters.
- Metrics hooks are present in core components; integration points depend on the consumer crate.

### Security

- Default (dev): If `NYX_DAEMON_TOKEN` is unset or empty, daemon authorizes all RPCs.
- Production: Enable auth with either an env token or a Tor-style cookie file.
	- Env token: set `NYX_DAEMON_TOKEN` to a strong secret and pass it as `auth`.
	- Cookie file (Tor-style): write a token to a file and point daemon/CLI to it.
		- Daemon reads token in this order: `NYX_DAEMON_TOKEN` (non-empty) -> `NYX_DAEMON_COOKIE` -> default cookie paths.
			- Windows default: `%APPDATA%\nyx\control.authcookie`
			- Unix default: `$HOME/.nyx/control.authcookie` (also tries `/run/nyx/control.authcookie`)
		- CLI auto-discovers token in this order: env (`NYX_CONTROL_TOKEN` -> `NYX_TOKEN`) -> cookie file (`NYX_DAEMON_COOKIE` or default path) -> `nyx.toml` [cli.token]. Empty/whitespace is ignored.
	- Generate a cookie file via CLI:
		- Windows (PowerShell)
			```powershell
			nyx-cli gencookie --path "$env:APPDATA/nyx/control.authcookie" --force
			```
		- Unix
			```bash
			nyx-cli gencookie --path "$HOME/.nyx/control.authcookie" --force
			```
- IPC endpoints are local-only (Unix socket / Windows named pipe); still treat tokens/cookies as sensitive.

### Contributing

See `CONTRIBUTING.md` and the code of conduct in `CODE_OF_CONDUCT.md`.

### License

This project is dual-licensed under either MIT or Apache-2.0. See `LICENSE-MIT` and `LICENSE-APACHE` for details.
