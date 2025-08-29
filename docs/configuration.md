# Configuration Reference

NyxNet uses a TOML file (default `nyx.toml`) and environment variables. The following is grounded in the repository’s `nyx.toml` and `examples/cmix_config.toml`.

## Root keys

- listen_port (u16): default 43300
- node_id (string): set to "auto" to generate at startup
- log_level: one of `trace|debug|info|warn|error`
- bootstrap_peers: array of multiaddr strings (optional; examples are placeholders)

## [network]

- bind_addr: e.g. "127.0.0.1:43300" (localhost by default for development)
- development (bool): toggles development mode

## [dht]

- enabled (bool): enable DHT peer discovery
- port (u16): DHT bind port
- peer_cache_size (usize): cache limit
- discovery_timeout (seconds)

## [endpoints]

- grpc_addr: e.g. "127.0.0.1:50051" (JSON-RPC is used; gRPC is disabled/reserved)
- prometheus_addr: e.g. "127.0.0.1:9090"; when set, exposes `/metrics`

## [cli]

- max_reconnect_attempts: cap for reconnect attempts

## Stream safety cap

- max_frame_len_bytes: e.g. 8_388_608. If present, applied to Nyx stream defaults at daemon boot.

## cMix example (from `examples/cmix_config.toml`)

[mix]
- mode = "cmix"
- batch_size = 100
- vdf_delay_ms = 100

Cover traffic:
- cover_traffic_rate = 10.0
- adaptive_cover = true
- target_utilization = 0.4 (typical range 0.2–0.6)

[multipath]
- enabled = true
- max_paths, min_hops, max_hops, reorder_timeout_ms, weight_method

## Environment variables

- NYX_DAEMON_TOKEN: top-priority auth token
- NYX_DAEMON_COOKIE: explicit cookie file path
- NYX_DAEMON_STRICT_AUTH: set to `1` to require a valid token for privileged ops
- NYX_PROMETHEUS_ADDR: e.g. `127.0.0.1:0` to expose metrics on a random port
- NYX_CONFIG: initial config file path
- RUST_LOG: overrides log level

## Best practices

- For local dev, prefer `bind_addr = "127.0.0.1:43300"`.
- For production, set `development = false`, set an explicit `bind_addr`, and protect Prometheus (localhost or network policy).
- Keep `max_frame_len_bytes` minimal to reduce attack surface.