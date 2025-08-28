# Nyx — U22 Submission Guide

This document summarizes what to run, what’s included, and what’s out-of-scope for the U22 Programming Contest submission.

## What’s included (scope)
- Core protocol crates: nyx-core, nyx-crypto, nyx-stream, nyx-transport, nyx-fec, nyx-telemetry
- Mix routing and adaptive cover traffic (nyx-mix)
- Daemon and CLI (Windows supported)
- Tests, benches (where applicable), CI-friendly formatting/linting
- Formal/conformance assets (TLA+, conformance tests)

## Out of scope for this submission
- Full Control Plane v1.0 feature set (advanced DHT/capability negotiation are under active development)
- QUIC end-to-end demos by default (gated/optional)
- WASM SDK browser features (subset only)
- Mobile FFI (iOS/Android) full integration
- Dynamic plugin loading in browsers

These are present as implementations, feature-gated modules, or design docs, but not required for the demo.

## Quick verify (Windows PowerShell)
Run a complete local verification (build + tests + lint + format check):

```powershell
# From the repository root
./scripts/build-verify.ps1
```

Equivalent individual commands:
```powershell
cargo build --workspace --release
cargo test --workspace --all-features
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
```

## Minimal demo flow
1) Build everything (release):
```powershell
cargo build --workspace --release
```
2) Start the daemon (new terminal recommended):
```powershell
cargo run -p nyx-daemon --release
```
3) In another terminal, run the CLI (help and config template):
```powershell
cargo run -p nyx-cli --release -- --help
# A sample config template also exists at the repo root: nyx.toml
```

Notes:
- The repo root already contains `nyx.toml`. You can point `NYX_CONFIG` env var to it if needed.
- QUIC, hybrid crypto, and other heavy tests are feature-gated; defaults focus on fast, portable verification.

## DHT full implementation quick demo (local)
Two local nodes exchanging key-value via DHT (pseudo code). For production use, signed RPC and persistence are enabled:

```rust
use nyx_control::dht::{DhtNode, DhtConfig, StorageKey, StorageValue};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let mut cfg1 = DhtConfig{ bind: "127.0.0.1:0".parse::<SocketAddr>()?, ..Default::default() };
	cfg1.persist_path = Some(std::path::PathBuf::from("./dht_snapshot1.cbor"));
	let mut n1 = DhtNode::spawn(cfg1).await?;

	let mut cfg2 = DhtConfig{ bind: "127.0.0.1:0".parse::<SocketAddr>()?, ..Default::default() };
	cfg2.persist_path = Some(std::path::PathBuf::from("./dht_snapshot2.cbor"));
	let n2 = DhtNode::spawn(cfg2).await?;
	n1.add_peer(n2.info()).await; // bootstrap

	let k = StorageKey::from_bytes(b"hello");
	let v = StorageValue::from_bytes(b"world");
	n1.put(k.clone(), v.clone()).await?;
	let got = n2.get(k.clone()).await?;
	assert_eq!(got, Some(v));
	// optional immediate snapshot
	n1.persist_snapshot().await?;
	Ok(())
}
```

Notes:
- RPC messages are signed (ed25519) and verified after the peer's public key is learned via a one-time GetPubKey request.
- Persistence: when DhtConfig.persist_path is set, snapshots are periodically saved and loaded on startup (TTL is respected on reload).
- For production, enable NAT traversal and discovery.

## Specs and design references
- spec/Nyx_Protocol_v0.1_Spec.md (and EN version)
- spec/Nyx_Protocol_v1.0_Spec.md (and EN version)
- spec/Nyx_Design_Document.md (and EN version)
- spec/Capability_Negotiation_Policy.md (and EN version)

## Quality gates (expected on a clean checkout)
- Build: PASS (workspace, release/dev)
- Tests: PASS (workspace)
- Lint: PASS (`cargo clippy -- -D warnings`)
- Format: PASS (`cargo fmt -- --check`)

## Acknowledgements and license
- Dual-licensed: MIT OR Apache-2.0 (see LICENSE files)
- See CODE_OF_CONDUCT.md and CONTRIBUTING.md for project guidelines

---
If reviewers need a one-liner: use `./scripts/build-verify.ps1`.
