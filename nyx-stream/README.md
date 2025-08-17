Nyx Stream: Plugin Framework Summary

Overview

- Plugin frame range: 0x50–0x5F
- Header format: CBOR-serialized `PluginHeader { id, flags, data }`
- Dispatcher enforces registration and per-frame permissions

Key modules

- plugin.rs: Core types, IDs, frame constants
- plugin_cbor.rs: Header decode helpers (+ tests)
- plugin_frame.rs: Full wire frame (encode/decode)
- plugin_registry.rs: Registry + permissions
- plugin_dispatch.rs: Safe dispatch, stats, logging hygiene (+ tests)
- plugin_handshake.rs: Minimal handshake payload helpers
- plugin_manifest.rs: TOML manifest + schema validation
- plugin_settings.rs: Runtime settings defaults
- plugin_ipc.rs / plugin_sandbox*.rs: Stubs for transport/sandbox

Run tests

PowerShell (Windows):

```powershell
cargo test -p nyx-stream
```

Notes

- All code in this crate avoids `unsafe`.
- Windows/macOS sandbox files are placeholders; production enforcement is platform crates’役割。
