# nyx-transport

Minimal, safe, std-only transport helpers for Nyx.

- UDP loopback endpoint
- Host:port validation (with IP literal fast-path)
- Local UDP echo (STUN-like) for smoke tests
- TCP fallback connector with timeout
- IPv4-mapped IPv6 helper (Teredo placeholder)
- ICE-like loopback candidate gathering
- QUIC feature-gated stub (no C deps)

## Examples

See tests in `src/` for usage patterns. The crate is intentionally kept small and self-contained to avoid external dependencies.
