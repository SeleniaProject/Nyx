# Specifications overview

This page summarizes the protocol/design specifications that live in the repository under `spec/`.

Files (English):
- `spec/Nyx_Protocol_v1.0_Spec_EN.md` — Protocol v1.0 (draft; includes planned features)
- `spec/Nyx_Protocol_v0.1_Spec_EN.md` — Protocol v0.1 (baseline implemented set)
- `spec/Capability_Negotiation_Policy_EN.md` — Capability negotiation policy
- `spec/Nyx_Design_Document_EN.md` — Design document

Files (Japanese):
- `spec/Nyx_Protocol_v1.0_Spec.md`
- `spec/Nyx_Protocol_v0.1_Spec.md`
- `spec/Capability_Negotiation_Policy.md`
- `spec/Nyx_Design_Document.md`

Note: v1.0 includes roadmap items. The codebase may implement a subset today; see README “Specifications” notes.

## Nyx Protocol v1.0 — highlights (draft)
- Multipath: per-packet PathID, extended header (12-byte CID), fixed 1280-byte payloads
- Hybrid PQ handshake: X25519 + Kyber; HPKE available; anti-replay window 2^20 per direction
- Plugin frames 0x50–0x5F with CBOR header `{id:u32, flags:u8, data:bytes}`
- Capability negotiation via CBOR capability list; unsupported Required → CLOSE 0x07 (with 4-byte ID)
- Optional cMix mode (batch ≈ 100, VDF ≈ 100ms), adaptive cover traffic (target utilization 0.2–0.6)
- Compliance levels: Core / Plus / Full; telemetry: OTLP + Prometheus

## Nyx Protocol v0.1 — baseline
- Core crypto (X25519 + AEAD), basic stream & management frames
- Fixed-size packets (1280B), FEC baseline
- Single-path data plane, UDP primary transport, TCP fallback

## Capability Negotiation Policy — essentials
- CBOR list of capabilities; each entry `{id, flags, data}`
- flags bit 0x01 = Required; otherwise Optional
- Negotiation fails fast if a Required ID is not supported (CLOSE 0x07 + unsupported ID)

## Design Document — themes
- Principles: security-by-design, performance without compromise, modularity, open development
- Layers: secure stream, mix routing, obfuscation + FEC, transport; async pipeline with backpressure
- Crypto: AEAD/KDF/HPKE, key rotation, PQ readiness; threat model includes global passive/active

---

For full details, open the files under `spec/` in the repository.
