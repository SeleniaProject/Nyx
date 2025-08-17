# nyx-daemon

A pure-Rust daemon providing Nyx control, configuration management, events, and metrics without unsafe code or C/C++ dependencies.

## Build

- Default
  - `cargo build -p nyx-daemon`
- With all features
  - `cargo build -p nyx-daemon --all-features`

## Run

- Windows (Named Pipe)
  - Listens on `\\.\pipe\nyx-daemon`
- Unix (Unix Domain Socket)
  - Listens on `/tmp/nyx.sock`

The daemon logs info-level messages by default; override with `RUST_LOG`.

## IPC Protocol

- Request/response over a single line of JSON (newline terminated)
- Basic ops:
  - `get_info`
  - `reload_config` (auth)
  - `update_config` (auth)
  - `list_config_versions` (auth)
  - `rollback_config` (auth)
  - `create_config_snapshot` (auth)
  - `subscribe_events` (auth) — switches connection into event stream mode

See `examples/ipc_client.rs` for a minimal client.

## Auth

- Token discovery order:
  1) `NYX_DAEMON_TOKEN` (if non-empty)
  2) Cookie file at `NYX_DAEMON_COOKIE` path or default user path
     - Windows: `%APPDATA%/nyx/control.authcookie`
     - Unix: `$HOME/.nyx/control.authcookie`
  3) If neither exists, a cookie is auto-generated on start.
- Strict mode: set `NYX_DAEMON_STRICT_AUTH=1` to require a valid token for privileged ops.

## Metrics

- Prometheus exporter (optional): set `NYX_PROMETHEUS_ADDR` to e.g. `127.0.0.1:0`
  - Exposes `/metrics` via an embedded HTTP server

## Configuration

- Initial config path via `NYX_CONFIG`
- On boot, if the config contains `max_frame_len_bytes`, Nyx stream default limit is updated.

## Development Notes

- Unsafe code is forbidden (`#![forbid(unsafe_code)]`)
- SIMD JSON parsing is disabled for decoding (safety); encoding may use `simd-json` when enabled.
- Run tests:
  - `cargo test -p nyx-daemon --all-features`
- Lint:
  - `cargo clippy -p nyx-daemon --all-features -- -D warnings`
