# Nyx WASM SDK Feature Matrix

| Capability | Status | Notes |
|------------|--------|-------|
| Noise Hybrid Handshake | Partial | Demo via `noise_handshake_demo`; not full session mgmt |
| HPKE | Not Implemented | Planned: expose hpke encrypt/decrypt when wasm-safe primitives ready |
| Multipath | Not Implemented | Browser lacks required low-level QUIC/UDP APIs |
| Plugin System | Not Implemented | Dynamic loading & sandbox not available in browser |
| Capability Negotiation | Not Implemented | Close codes not surfaced in WASM yet |
| Push Registration | Implemented (basic) | `nyx_register_push` returns endpoint only |
| Close Code Mapping | Not Implemented | Would require protocol layer in WASM |
| Reconnect Logic | Not Implemented | Session abstraction absent |
| Telemetry Hooks | Planned | To integrate with browser Performance API |

_Last updated: 2025-08-10_
