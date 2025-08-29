# IPC/API Reference (newline-delimited JSON)

Nyx daemon exposes IPC as newline-delimited JSON request/response (one JSON object per line).

## Endpoints

- Unix: `/tmp/nyx.sock`
- Windows: `\\.\\pipe\\nyx-daemon`

## Authentication

Token discovery order:

1) `NYX_DAEMON_TOKEN`
2) `NYX_DAEMON_COOKIE` file path (or the default path below)
   - Windows: `%APPDATA%/nyx/control.authcookie`
   - Unix: `$HOME/.nyx/control.authcookie`
3) If none is found, a token is generated at boot.

Set `NYX_DAEMON_STRICT_AUTH=1` to require a valid token for privileged operations.

## Request shape

```json
{ "op": "get_info" }
```

Privileged operations must include a `token` field:

```json
{ "op": "reload_config", "token": "<secret>", "path": "nyx.toml" }
```

## Operations

- get_info: return runtime information
- reload_config (auth): reload configuration file
- update_config (auth): patch configuration
- list_config_versions (auth): list config versions
- rollback_config (auth): rollback to a specific version
- create_config_snapshot (auth): create a snapshot
- subscribe_events (auth): switch the connection to event-stream mode

## Response examples

Success:

```json
{ "ok": true, "data": { "version": "1.0.0", "pid": 12345 } }
```

Error:

```json
{ "ok": false, "error": { "code": "InvalidToken", "message": "token mismatch" } }
```

## Metrics

If `NYX_PROMETHEUS_ADDR` is set, an embedded HTTP server exposes Prometheus metrics at `/metrics`.