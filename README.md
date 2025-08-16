## Nyx Protocol 🌓

![CI](https://github.com/SeleniaProject/Nyx/actions/workflows/comprehensive_ci.yml/badge.svg)
![Formal Verification](https://github.com/SeleniaProject/Nyx/actions/workflows/formal-verification.yml/badge.svg)
![Docs](https://github.com/SeleniaProject/Nyx/actions/workflows/docs.yml/badge.svg)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE-MIT)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache--2.0-blue.svg)](LICENSE-APACHE)
![Rust Edition](https://img.shields.io/badge/Rust-2021-orange)
![OS](https://img.shields.io/badge/OS-Linux%20%7C%20macOS%20%7C%20Windows-555)

A modular, privacy-focused transport protocol implemented in Rust. Nyx combines a secure streaming layer, UDP transport with NAT traversal, a mix routing layer, rich telemetry, and formal verification into one cohesive workspace.

> Dual-licensed under MIT and Apache-2.0. Actively developed in a multi-crate Cargo workspace.

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

### Highlights

- 🔐 Secure stream layer with framing and flow control (`nyx-stream`)
- 🛰️ UDP transport adapter with NAT traversal utilities (`nyx-transport`)
- 🧭 Mix routing layer for path selection and cover traffic (`nyx-mix`)
- 🧩 Capability negotiation and control plane (DHT, settings sync) (`nyx-control`)
- ⚙️ Cryptography engine with Noise_Nyx handshake and HKDF helpers (`nyx-crypto`)
- 🧱 Reed–Solomon FEC for fixed 1280B packets (`nyx-fec`)
- 📊 Telemetry via OpenTelemetry OTLP and Prometheus (`nyx-telemetry`)
- 🧰 CLI tools and long-running daemon (`nyx-cli`, `nyx-daemon`)
- 📦 SDKs for Rust and WebAssembly + mobile FFI bindings (`nyx-sdk`, `nyx-sdk-wasm`, `nyx-mobile-ffi`)
- ✅ Conformance tests and formal models (TLA+) with automated verification (`nyx-conformance`, `formal/`, `scripts/`)

### Architecture

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
				|  nyx-mobile-ffi  |  nyx-cli (WIP)  |
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

### Workspace layout (selected crates)

- `nyx-core` — Core types, config, errors
- `nyx-crypto` — Noise_Nyx handshake, crypto primitives wrappers
- `nyx-stream` — Secure stream layer (framing, flow control)
- `nyx-transport` — UDP transport + NAT traversal
- `nyx-mix` — Mix routing (path selection, cover traffic)
- `nyx-fec` — Reed–Solomon FEC (1280B fixed packets)
- `nyx-telemetry` — OpenTelemetry OTLP + Prometheus exporters
- `nyx-control` — Control plane (DHT, settings sync)
- `nyx-daemon` / `nyx-cli` — Daemon and command line tools
- `nyx-sdk`, `nyx-sdk-wasm`, `nyx-mobile-ffi` — SDKs and platform bindings
- `nyx-conformance` — Protocol conformance test suite

### Build and run

Requirements:

- Rust (stable) and Cargo
- Protobuf codegen is handled in-repo via a vendored helper; a system `protoc` is optional.

Common tasks:

```powershell
# Build all crates in release mode
cargo build --release

# Run the daemon (IPC: Unix socket or Windows named pipe)
cargo run -p nyx-daemon --release

# Run tests across the workspace
cargo test --workspace

# (Optional, Windows) Build & verification helper
./scripts/build-verify.ps1
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

### Status

- Actively developed. Multi-crate workspace builds and tests pass across platforms.
- `nyx-cli` is currently a placeholder entrypoint (WIP).
- Documentation site under refactor; refer to `spec/` for authoritative protocol details.

### OS Support

- Linux/macOS: Unix domain socket at `/tmp/nyx.sock`
- Windows: Named pipe at `\\.\\pipe\\nyx-daemon`
- CI workflows cover Linux, Windows, and (where applicable) docs generation; formal verification runs in CI.

### Roadmap

- Core protocol hardening and conformance coverage expansion (`nyx-conformance`)
- CLI feature implementation and UX
- Extended telemetry dashboards (Grafana) and OTLP pipelines
- SDK ergonomics and examples (Rust/WASM/Mobile)
- Additional protocol plugins per v1.0 spec and capability negotiation policy

### Configuration

- `NYX_CONFIG`: Optional path to a config file that the daemon will manage.
- Configuration can be hot-reloaded and snapshotted via daemon RPC:
	- `reload_config`, `update_config { settings }`, `create_config_snapshot`,
		`list_config_versions`, `rollback_config { version }`.

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
