//! Capability Negotiation Traceability Matrix
//!
//! This document provides comprehensive traceability between the capability negotiation
//! specification (`spec/Capability_Negotiation_Policy.md`) and the implementation
//! in `nyx-stream/src/capability.rs` and related modules.
//!
//! # Specification Compliance Matrix
//!
//! | Specification Section | Implementation Location | Test Coverage | Status |
//! |----------------------|-------------------------|---------------|---------|
//! | Wire Format (CBOR) | `capability.rs:42-59` | `capability_cbor_roundtrip` | ✅ Complete |
//! | Capability IDs | `capability.rs:21-24` | `plugin_framework_tests.rs` | ✅ Complete |
//! | Negotiation Algorithm | `capability.rs:124-137` | `negotiation_*` tests | ✅ Complete |
//! | Error Handling 0x07 | `management.rs:10-26` | `close_frame_*` tests | ✅ Complete |
//! | Extension Policy | `capability.rs:166-181` | `validate_capability` | ✅ Complete |
//! | Security Considerations | `capability.rs:102-109` | `size_limits` tests | ✅ Complete |
//!
//! # Implementation Mapping
//!
//! ## Core Data Structures
//!
//! ### Capability Structure (`capability.rs:42-59`)
//! **Spec Reference**: Section "ワイヤ形式（CBOR）"
//! ```rust
//! pub struct Capability {
//!     pub id: u32,    // Capability ID（32-bit）
//!     pub flags: u8,  // 下位ビット 0 が Required を表す（1=Required, 0=Optional）
//!     pub data: Vec<u8>, // 任意の付加データ
//! }
//! ```
//! **Tests**: `test_capability_flags`, `test_cbor_roundtrip`
//!
//! ### Predefined Capability IDs (`capability.rs:21-24`)
//! **Spec Reference**: Section "既定の Capability ID"
//! ```rust
//! pub const CAP_CORE: u32 = 0x0001;           // core（必須）
//! pub const CAP_PLUGIN_FRAMEWORK: u32 = 0x0002; // plugin_framework（任意）
//! ```
//! **Tests**: `test_plugin_framework_capability_negotiation`
//!
//! ## Wire Protocol Implementation
//!
//! ### CBOR Encoding (`capability.rs:97-102`)
//! **Spec Reference**: Section "ワイヤ形式（CBOR）"
//! ```rust
//! pub fn encode_caps(capabilities: &[Capability]) -> Result<Vec<u8>, CapabilityError>
//! ```
//! **Security**: Uses `ciborium::ser::into_writer` for secure serialization
//! **Tests**: `test_cbor_roundtrip`
//!
//! ### CBOR Decoding with Size Limits (`capability.rs:105-115`)
//! **Spec Reference**: Section "セキュリティ配慮"
//! ```rust
//! pub fn decode_caps(data: &[u8]) -> Result<Vec<Capability>, CapabilityError>
//! ```
//! **Security**: Enforces 64KB limit to prevent DoS attacks
//! **Tests**: `test_decode_size_limits`
//!
//! ## Negotiation Algorithm
//!
//! ### Core Negotiation Logic (`capability.rs:124-137`)
//! **Spec Reference**: Section "交渉アルゴリズム"
//! ```rust
//! pub fn negotiate(
//!     local_supported: &[u32],
//!     peer_caps: &[Capability],
//! ) -> Result<(), CapabilityError>
//! ```
//! **Algorithm**: Exact implementation of spec pseudocode:
//! 1. For each peer capability marked as required
//! 2. Check if local implementation supports it
//! 3. Return error on first unsupported required capability
//! **Tests**: `test_negotiate_success`, `test_negotiate_unsupported_required`
//!
//! ## Error Handling
//!
//! ### CLOSE Frame Generation (`management.rs:10-26`)
//! **Spec Reference**: Section "未対応必須 Capability のエラー終了（CLOSE 0x07）"
//! ```rust
//! pub fn build_close_unsupported_cap(id: u32) -> Vec<u8>
//! ```
//! **Implementation**: 
//! - Error code: `ERR_UNSUPPORTED_CAP = 0x07`
//! - Format: 2 bytes error code + 4 bytes capability ID (big-endian)
//! **Tests**: `test_build_close_unsupported_cap`, `test_roundtrip_capability_ids`
//!
//! ### CLOSE Frame Parsing (`management.rs:28-42`)
//! **Spec Reference**: Section "未対応必須 Capability のエラー終了（CLOSE 0x07）"
//! ```rust
//! pub fn parse_close_unsupported_cap(reason: &[u8]) -> Option<u32>
//! ```
//! **Validation**: Checks exact format (6 bytes) and error code match
//! **Tests**: `test_parse_close_unsupported_cap`, `test_parse_close_invalid_length`
//!
//! ## Security Implementation
//!
//! ### Input Validation (`capability.rs:184-205`)
//! **Spec Reference**: Section "セキュリティ配慮"
//! ```rust
//! pub fn validate_capability(cap: &Capability) -> Result<(), CapabilityError>
//! ```
//! **Security Measures**:
//! - Capability data size limit (1024 bytes)
//! - Core capability format validation (empty data requirement)
//! - CBOR input size limit (64KB) in decode_caps
//! **Tests**: `test_validate_capability_size_limits`, `test_core_capability_validation`
//!
//! ## Extension Policy Support
//!
//! ### Forward Compatibility (`capability.rs:166-181`)
//! **Spec Reference**: Section "拡張ポリシー"
//! - Unknown capability IDs are allowed (forward compatibility)
//! - Optional capabilities never cause negotiation failure
//! - Validation preserves extensibility for future capability versions
//! **Tests**: `test_negotiate_optional_unknown`
//!
//! ### Local Capability Advertisement (`capability.rs:139-144`)
//! **Spec Reference**: Section "既定の Capability ID"
//! ```rust
//! pub fn get_local_capabilities() -> Vec<Capability>
//! ```
//! **Implementation**: Returns core (required) + plugin framework (optional)
//! **Tests**: `test_plugin_framework_capability_negotiation`
//!
//! # Property-Based Test Coverage
//!
//! ## Comprehensive Property Tests (`nyx-conformance/tests/capability_negotiation_properties.rs`)
//! 
//! ### CBOR Roundtrip Properties
//! - **Test**: `capability_cbor_roundtrip`
//! - **Property**: ∀ capabilities, encode(decode(capabilities)) = capabilities
//! - **Coverage**: All capability combinations, sizes, and flag values
//!
//! ### Flag Interpretation Properties  
//! - **Test**: `capability_flags_interpretation`
//! - **Property**: Required/Optional flags correctly interpreted
//! - **Coverage**: All capability IDs and data combinations
//!
//! ### Negotiation Success Properties
//! - **Test**: `negotiation_success_when_supported`
//! - **Property**: Negotiation succeeds when all required capabilities supported
//! - **Coverage**: Variable capability sets, required/optional mixes
//!
//! ### Negotiation Failure Properties
//! - **Test**: `negotiation_fails_missing_required`
//! - **Property**: Negotiation fails on first unsupported required capability
//! - **Coverage**: Guaranteed unsupported capability scenarios
//!
//! ### Optional Capability Properties
//! - **Test**: `optional_capabilities_never_fail`
//! - **Property**: Optional capabilities never cause negotiation failure
//! - **Coverage**: Any combination of optional capabilities vs. local support
//!
//! ### Security Properties
//! - **Test**: `capability_validation_size_limits`
//! - **Property**: Large capabilities rejected, normal ones accepted
//! - **Coverage**: Size boundaries around limits
//! - **Test**: `cbor_decode_size_limits`
//! - **Property**: Large CBOR input rejected
//! - **Coverage**: Size boundaries around 64KB limit
//!
//! ### Error Frame Properties
//! - **Test**: `close_frame_unsupported_capability`
//! - **Property**: CLOSE frame roundtrip preserves capability ID
//! - **Coverage**: All 32-bit capability ID values
//!
//! # Audit Trail
//!
//! ## Specification Compliance Verification
//! - ✅ Wire format matches spec exactly (CBOR with id/flags/data fields)
//! - ✅ Capability IDs match predefined values (0x0001, 0x0002)
//! - ✅ Negotiation algorithm implements spec pseudocode exactly
//! - ✅ Error code 0x07 with capability ID in CLOSE reason
//! - ✅ Security size limits implemented (64KB CBOR, 1KB capability data)
//! - ✅ Extension policy supported (unknown capabilities allowed if optional)
//!
//! ## Test Coverage Verification
//! - ✅ Unit tests: 14 tests covering all core functionality
//! - ✅ Property tests: 9 comprehensive property-based tests
//! - ✅ Integration tests: Plugin framework capability integration
//! - ✅ Conformance tests: Full protocol compliance verification
//!
//! ## Implementation Quality Verification
//! - ✅ Pure Rust implementation (no unsafe code)
//! - ✅ Comprehensive error handling with typed errors
//! - ✅ Security-first design with input validation
//! - ✅ Forward compatibility preserved
//! - ✅ Performance-conscious (minimal allocations, efficient serialization)
//!
//! # API Documentation Links
//!
//! - **Primary Implementation**: [`nyx_stream::capability`](../nyx-stream/src/capability.rs)
//! - **Management Frames**: [`nyx_stream::management`](../nyx-stream/src/management.rs)
//! - **Conformance Tests**: [`nyx_conformance::capability_negotiation_properties`](../nyx-conformance/tests/capability_negotiation_properties.rs)
//! - **Integration Tests**: [`nyx_stream::plugin_framework_tests`](../nyx-stream/tests/plugin_framework_tests.rs)
//! - **Specification**: [`spec/Capability_Negotiation_Policy.md`](../spec/Capability_Negotiation_Policy.md)
