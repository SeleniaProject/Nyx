# Plugin Sandbox Implementation Guide

## Overview

The Nyx Plugin Sandbox provides cross-platform process isolation for plugin execution. This implementation focuses on cooperative security using Pure Rust without C/C++ dependencies, providing practical security benefits while maintaining memory safety.

## Architecture

### Multi-Layer Security Model

1. **Application Layer**: Plugin framework checks (`nyx-stream/src/plugin_sandbox.rs`)
   - Network host allowlist verification
   - File path prefix validation
   - Platform-agnostic path normalization

2. **OS Integration Layer**: Process-level restrictions (`nyx-core/src/sandbox.rs`)
   - Resource limits (memory, file descriptors, process count)
   - Environment-based cooperative restrictions
   - Platform-specific implementations

### Platform Support

#### Windows
- **Implementation**: Job Objects via `win32job` crate
- **Features**: KillOnJobClose, ActiveProcessLimit=1
- **Status**: Production ready

#### Linux
- **Implementation**: Resource limits via `nix` crate
- **Features**: RLIMIT_NPROC, RLIMIT_NOFILE, RLIMIT_AS restrictions
- **Cooperation**: Environment variables for plugin behavior
- **Status**: Cooperative restrictions implemented

#### macOS
- **Implementation**: Resource limits via `nix` crate
- **Features**: Similar to Linux with macOS-specific considerations
- **Cooperation**: Environment variables for plugin behavior
- **Status**: Cooperative restrictions implemented

#### OpenBSD
- **Implementation**: pledge/unveil system calls
- **Features**: Fine-grained capability restrictions
- **Status**: Available when feature enabled

## Usage

### Basic Usage

```rust
use nyx_core::sandbox::{apply_policy, SandboxPolicy, SandboxStatus};

// Apply minimal restrictions suitable for most plugins
let status = apply_policy(SandboxPolicy::Minimal);
match status {
    SandboxStatus::Applied => println!("Sandbox active"),
    SandboxStatus::Unsupported => println!("Sandbox not available on this platform"),
}
```

### Plugin Framework Integration

```rust
use nyx_stream::plugin_sandbox::{SandboxPolicy, SandboxGuard};

let policy = SandboxPolicy::locked_down()
    .allow_connect_host("api.example.com")
    .allow_path_prefix(Path::new("/tmp/nyx"));

let guard = SandboxGuard::new(policy)?;
// Plugin operations are now restricted
```

## Security Properties

### Achieved Protection

1. **Resource Exhaustion**: Process and memory limits prevent fork bombs and DoS
2. **File System**: Path prefix restrictions limit file access
3. **Network**: Host allowlist restricts network connections
4. **Process Control**: Prevention of subprocess spawning

### Limitations

- **Cooperative Model**: Relies on plugins respecting environment restrictions
- **Pure Rust Focus**: Avoids kernel-level enforcement to maintain C/C++ independence
- **Platform Differences**: Feature parity varies by OS capabilities

## Testing

Run comprehensive sandbox tests:

```bash
# Test all platforms with sandbox features
cargo test -p nyx-stream sandbox
cargo test -p nyx-core --features os_sandbox sandbox

# Test specific platform integration
cargo test -p nyx-stream --test plugin_sandbox_integration
```

## Configuration

### Environment Variables

The sandbox sets these environment variables for plugin cooperation:

- `SANDBOX_POLICY`: "minimal" or "strict"
- `NO_SUBPROCESS`: "1" to prevent subprocess creation
- `NO_NETWORK`: "1" to prevent network operations (strict mode)
- `NO_FILESYSTEM_WRITE`: "1" to prevent filesystem writes (strict mode on macOS)

### Resource Limits

Default limits applied across platforms:

- **Processes**: 10 soft limit, 50 hard limit
- **File Descriptors**: 64 soft limit, 128 hard limit  
- **Memory**: 64MB soft limit, 128MB hard limit

## Future Enhancements

1. **Enhanced Kernel Integration**: Optional kernel-level enforcement
2. **Dynamic Policy Updates**: Runtime policy modification
3. **Audit Logging**: Comprehensive security event logging
4. **Container Integration**: Docker/Podman sandbox backends

## Troubleshooting

### Common Issues

1. **Sandbox Unsupported**: Check feature flags and platform support
2. **Resource Limits Too Restrictive**: Adjust limits in implementation
3. **Plugin Compatibility**: Ensure plugins respect environment variables

### Debugging

Enable debug logging to see sandbox application status:

```bash
RUST_LOG=debug cargo test sandbox
```

## Security Considerations

This implementation prioritizes:

1. **Memory Safety**: Pure Rust implementation
2. **Practical Security**: Realistic threat model for plugin systems
3. **Cross-Platform**: Consistent behavior across operating systems
4. **Maintainability**: No complex C FFI or kernel modules

The cooperative model is suitable for trusted plugin environments where complete isolation is not required, but resource limits and basic restrictions provide meaningful security benefits.
