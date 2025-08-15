# Daemon Plain IPC Protocol

This document describes the Nyx daemon control plane over a simple, pure-Rust IPC channel.

- Transport: newline-delimited JSON (NDJSON)
- Endpoint:
  - Unix: `/tmp/nyx.sock` (Unix domain socket)
  - Windows: `\\.\pipe\nyx-daemon` (Named Pipe)
- Auth: optional static token via env `NYX_DAEMON_TOKEN`. When set, privileged ops require `auth` field.
- Framing: each request is a single JSON object terminated by `\n`. Responses and streamed events are one JSON object per line.

## Envelope

All requests are wrapped in an object with optional request id and auth token:

```
{
  "id": "req-123",        // optional
  "auth": "<token>",       // optional, required for privileged ops when NYX_DAEMON_TOKEN is set
  "op": "get_info"         // operation (snake_case)
  // ... op-specific fields below ...
}
```

Responses echo the `id` when provided and use a uniform envelope:

```
{
  "ok": true,               // boolean success
  "code": 0,                // 0 on success, non-zero error code otherwise
  "id": "req-123",         // optional, echoed back if present in request
  "data": { ... },          // operation-specific data on success
  "error": "..."           // error message on failure
}
```

On rare JSON serialization failures, the daemon falls back to the same envelope and preserves `id` when possible.

## Operations

- get_info
  - Request: `{ "op": "get_info" }`
  - Data:
    - `node_id: string (hex)`
    - `version: string`
    - `uptime_sec: u32`

- reload_config (privileged)
  - Request: `{ "op": "reload_config" }`
  - Data: `ConfigResponse`

- update_config (privileged)
  - Request: `{ "op": "update_config", "settings": { "log_level": "info" } }`
  - Data: `ConfigResponse`

- list_config_versions (privileged)
  - Request: `{ "op": "list_config_versions" }`
  - Data: `VersionSummary[]`

- rollback_config (privileged)
  - Request: `{ "op": "rollback_config", "version": 3 }`
  - Data: `ConfigResponse`

- subscribe_events (privileged)
  - Request: `{ "op": "subscribe_events", "types": ["system", "metrics"] }`
  - Response: one-line ack `{ "ok":true, ... "data": {"subscribed": true} }` then a stream of `Event` objects

### Types

- ConfigResponse: `{ success: bool, message: string, validation_errors: string[] }`
- VersionSummary: `{ version: u64, timestamp: { "secs_since_epoch": u64, "nanos_since_epoch": u32 }, description: string }`
- Event: `{ ty: string, detail: string }`

Notes:
- When `NYX_DAEMON_TOKEN` is unset, all operations are allowed (development mode).
- When set, include `auth: "<token>"` for privileged operations.

## Minimal client example

See `nyx-daemon/examples/ipc_client.rs` for a small async client that sends a JSON request and prints responses/events.
