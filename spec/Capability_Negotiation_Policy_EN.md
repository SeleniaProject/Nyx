Capability Negotiation Policy (Nyx Protocol v1.0)

This document defines the rules and the forward-compatibility policy for capability negotiation in the Nyx Protocol. The reference implementation lives in the `nyx-stream` crate with conformance tests in this repository.

Goals
- Agree on a set of features (capabilities) supported by both endpoints
- Fail fast and interoperably when a required capability is not supported
- Provide clear rules for future extensions (backward/forward compatibility)

Terminology
- Required: If not supported by the peer, the connection must be closed
- Optional: If not supported, the connection continues without the feature

Wire Format (CBOR)
Capabilities are exchanged in the first CRYPTO-equivalent frame as a CBOR array of maps:

{ id: u32, flags: u8, data: bytes }

- id: capability identifier (32-bit)
- flags: LSB (0x01) indicates Required; 0 means Optional
- data: optional extension payload (version/parameters/etc.) as bytes

Implementation: `nyx-stream/src/capability.rs` (`Capability`, `encode_caps`, `decode_caps`)

Default Capability IDs
- 0x0001 = core (Required)
- 0x0002 = plugin_framework (Optional)

Defined in code: `LOCAL_CAP_IDS` in `nyx-stream/src/capability.rs`.

Negotiation Algorithm
For each Required capability advertised by the peer, check if the local stack supports it. The first unsupported Required capability triggers a negotiation failure.

fn negotiate(local_supported: &[u32], peer_caps: &[Capability]) -> Result<(), Unsupported(id)> {
  for cap in peer_caps {
    if cap.is_required() && !local_supported.contains(&cap.id) {
      return Err(Unsupported(cap.id))
    }
  }
  Ok(())
}

Implementation: `nyx-stream/src/capability.rs::negotiate`

Unsupported Required Capability → CLOSE 0x07
If a Required capability is not supported, close the connection with `ERR_UNSUPPORTED_CAP = 0x07` and include the 4-byte big-endian unsupported ID in the CLOSE reason.

- Constant/Builder: `nyx-stream/src/management.rs`
  - ERR_UNSUPPORTED_CAP: u16 = 0x07
  - build_close_unsupported_cap(id: u32) -> Vec<u8>

Extension Policy
1. Adding new capabilities
   - Allocate a new ID. Legacy stacks will treat it as unsupported.
   - Prefer Optional by default to avoid ecosystem splits. Promote to Required after wide deployment if necessary.
2. Versioning
   - Put self-defined schema into data, e.g. {version:u16, params:...}.
   - Unknown data must be safely ignorable.
3. Compatibility
   - Optional must never break connectivity; it only reduces feature scope.
   - Required is a hard fail with CLOSE 0x07 when missing.

Security Considerations
- Strictly validate CBOR sizes and field boundaries.
- Unknown Optional capabilities are ignored; unknown Required capabilities cause 0x07.
- Keep CLOSE reason minimal (4 bytes) to avoid misuse.

Implementation/Tests
- Implementation
  - `nyx-stream/src/capability.rs`
  - `nyx-stream/src/management.rs`
- Tests
  - `nyx-conformance/tests/capability_negotiation_properties.rs`
  - `nyx-stream/tests/plugin_framework_tests.rs`

Changelog
- Initial version: v1.0


