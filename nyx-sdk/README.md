# nyx-sdk

Application-facing SDK for Nyx. Provides:
- Error/Result types
- Lightweight async stream adapter over `nyx-stream`
- Daemon client helpers for JSON RPC over UDS/Named Pipe
- JSON-serializable API models (proto)

## Quick start

```rust
use nyx_sdk::NyxStream;
use bytes::Bytes;

#[tokio::main]
async fn main() -> nyx_sdk::Result<()> {
    let (a, b) = NyxStream::pair(1);
    a.send(Bytes::from_static(b"hello")).await?;
    let got = b.recv(50).await?;
    assert_eq!(got.unwrap(), Bytes::from_static(b"hello"));
    Ok(())
}
```

Daemon helpers (token discovery prefers `NYX_CONTROL_TOKEN` → `NYX_TOKEN` → cookie file):
```rust
use nyx_sdk::{DaemonClient, SdkConfig};

#[tokio::main]
async fn main() -> nyx_sdk::Result<()> {
    let cfg = SdkConfig::default();
    let client = DaemonClient::new_with_auto_token(cfg).await;
    // Example call (will attempt to connect to daemon endpoint):
    // let info = client.get_info().await?;
    Ok(())
}
```

## Features
- `reconnect`: enable backoff policy utilities.
- `metrics`: integrates with `nyx-core/telemetry`.
- `grpc-backup`: Legacy compatibility feature flag. gRPC functionality is intentionally disabled to avoid C dependencies (ring/openssl). Nyx uses pure Rust JSON-RPC over Unix Domain Sockets or Named Pipes for daemon communication.

## License
MIT OR Apache-2.0
