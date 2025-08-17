# Nyx Protocol Compliance Matrix CI/CD

This document describes the automated compliance validation system for the Nyx protocol implementation.

## Overview

The compliance matrix system provides automated verification that the implementation meets the requirements for different compliance levels as defined in the Nyx Protocol Specification.

## Compliance Levels

### Core Compliance
- **Purpose**: Minimum feature set for basic interoperability
- **Required Features**:
  - Stream management
  - Frame codec
  - Flow control  
  - Basic cryptography
- **CI Requirement**: MANDATORY - builds fail if not achieved

### Plus Compliance  
- **Purpose**: Enhanced features including multipath and post-quantum security
- **Required Features**:
  - All Core features
  - Multipath routing
  - Hybrid post-quantum cryptography
  - Capability negotiation
- **CI Requirement**: OPTIONAL - reported but non-blocking

### Full Compliance
- **Purpose**: Complete protocol implementation with all advanced features
- **Required Features**:
  - All Plus features
  - cMix integration
  - Plugin framework
  - Low power mode
  - VDF (Verifiable Delay Function)
- **CI Requirement**: OPTIONAL - reported but non-blocking

## CI/CD Integration

### Environment Variables

Configure compliance checking with these environment variables:

```bash
# Required compliance level (core|plus|full) - defaults to "core"
export NYX_REQUIRED_COMPLIANCE_LEVEL="core"

# Output directory for compliance reports and badges
export NYX_CI_OUTPUT_DIR="./compliance-reports"
```

### GitHub Actions Workflow

The `.github/workflows/compliance-matrix.yml` workflow provides:

1. **Core Compliance Gate**: Mandatory validation for all builds
2. **Compliance Matrix**: Test all levels with different feature combinations
3. **Badge Generation**: Create status badges for documentation
4. **Regression Detection**: Catch compliance regressions over time
5. **Cross-Platform**: Verify compliance on Linux, Windows, and macOS

### Running Locally

Test compliance locally:

```bash
# Test core compliance (mandatory)
cargo test --package nyx-conformance ci_compliance_gate_main --features hybrid

# Run full compliance matrix
cargo test --package nyx-conformance ci_compliance_matrix_full --features hybrid,multipath,telemetry,fec

# Generate compliance badges
mkdir -p ./badges
NYX_CI_OUTPUT_DIR=./badges cargo test --package nyx-conformance ci_compliance_matrix_full --features hybrid,multipath,telemetry,fec
```

## Feature Detection

The system automatically detects available features based on:

1. **Compile-time Cargo features**: Used for optional components
2. **Runtime capability detection**: For dynamic feature availability
3. **Module availability**: Check if required modules are compiled

### Core Features (Always Available)
- `stream` - Stream management
- `frame_codec` - Protocol frame encoding/decoding
- `flow_control` - Flow control mechanisms
- `basic_crypto` - Basic cryptographic operations
- `congestion_control` - Network congestion management
- `error_recovery` - Error handling and recovery
- `capability_negotiation` - Protocol capability negotiation
- `adaptive_cover_traffic` - Traffic analysis resistance

### Conditional Features (Cargo Feature-Gated)
- `multipath` - Multi-path routing (feature: "multipath")
- `hybrid_pq` - Post-quantum cryptography (feature: "hybrid")
- `telemetry` - Telemetry collection (feature: "telemetry")
- `fec` - Forward Error Correction (feature: "fec")
- `cmix` - cMix integration (features: "cmix" + "vdf")
- `vdf` - Verifiable Delay Function (feature: "vdf")
- `plugin_framework` - Plugin system (feature: "plugin")
- `low_power_mode` - Mobile/IoT optimizations (feature: "mobile")
- `quic_transport` - QUIC transport protocol (feature: "quic")
- `advanced_telemetry` - Enhanced telemetry (features: "telemetry" + "otlp")

## Compliance Reports

### JSON Reports

The system generates detailed JSON reports:

- `compliance_matrix.json` - Full compliance status matrix
- `feature_status.json` - Individual feature availability
- `hierarchy_validation.json` - Compliance level hierarchy validation
- `detailed_compliance_report.json` - Comprehensive analysis
- `compliance_failure.json` - Detailed failure diagnostics (on failure)

### Badge Generation

Compliance badges are generated in multiple formats:

- `compliance_badge.json` - Shields.io compatible badge data
- `compliance_badges.md` - Markdown with embedded badges

Example badges:
```markdown
![Compliance Level](https://img.shields.io/badge/Compliance-Plus-blue)
![Core](https://img.shields.io/badge/Core-passing-green)
![Plus](https://img.shields.io/badge/Plus-passing-green)  
![Full](https://img.shields.io/badge/Full-failing-red)
```

## Integration Examples

### GitHub Actions Status Check

```yaml
- name: Enforce Core Compliance
  run: |
    NYX_REQUIRED_COMPLIANCE_LEVEL=core \
    cargo test --package nyx-conformance ci_compliance_gate_main --features hybrid
```

### Docker Build Gate

```dockerfile
RUN NYX_REQUIRED_COMPLIANCE_LEVEL=core \
    cargo test --package nyx-conformance ci_compliance_gate_main --features hybrid
```

### Development Workflow

```bash
# Before submitting PR
make compliance-check

# Detailed compliance analysis  
make compliance-report

# Update documentation badges
make compliance-badges
```

## Troubleshooting

### Common Issues

1. **Core Compliance Failure**
   - Check that all core features are properly compiled
   - Verify no required modules are excluded from build
   - Review feature detection logic in `nyx-core/src/compliance.rs`

2. **Feature Detection Mismatch**
   - Ensure Cargo features match expected compilation flags
   - Check conditional compilation directives (`#[cfg(...)]`)
   - Verify feature detector includes new features

3. **CI Environment Issues**
   - Check environment variables are properly set
   - Verify output directory has write permissions
   - Ensure all required dependencies are available

### Debug Commands

```bash
# Verbose compliance checking
RUST_LOG=debug cargo test --package nyx-conformance -- --nocapture

# Feature-specific testing
cargo test --package nyx-conformance ci_feature_compilation_verification --features hybrid -- --nocapture

# Hierarchy validation
cargo test --package nyx-conformance ci_compliance_hierarchy_validation --features hybrid -- --nocapture
```

## Extending the System

### Adding New Features

1. Update `FeatureDetector::new()` in `nyx-core/src/compliance.rs`
2. Add feature to appropriate compliance level requirements
3. Update CI workflow feature matrices
4. Add feature documentation to this file

### Adding New Compliance Levels

1. Extend `ComplianceLevel` enum
2. Implement `ComplianceRequirements` method for new level
3. Update hierarchy validation logic
4. Add CI workflow support

### Custom Compliance Policies

Organizations can extend the compliance system:

1. Create custom compliance requirements
2. Implement organization-specific feature detection
3. Add custom validation tests
4. Integrate with existing CI/CD pipelines

## Security Considerations

- Compliance validation should be performed in trusted CI environments
- Feature detection may reveal implementation details
- Reports may contain sensitive configuration information
- Badge generation should use secure, signed artifacts

## Maintenance

### Regular Tasks

- Review and update feature requirements as protocol evolves
- Validate cross-platform compliance detection accuracy
- Monitor CI performance and optimize test execution
- Update documentation when new features are added

### Version Compatibility

The compliance system maintains compatibility across protocol versions:
- Core compliance requirements remain stable
- Plus/Full levels may evolve with new protocol versions  
- Feature detection adapts to implementation changes
- Backward compatibility is preserved for existing deployments
