# Plugin Sandbox Implementation

This document describes the cross-platform sandbox implementation for the Nyx Plugin Framework.

## Overview

The sandbox system provides two complementary layers of protection:

1. **Application-level Policy Guards** (`nyx-stream/src/plugin_sandbox.rs`)
   - Network and filesystem access validation before plugin operations
   - Host allowlists and path prefix restrictions
   - Cooperative enforcement for plugins using Nyx API

2. **OS-level Process Sandboxing** (`nyx-core/src/sandbox.rs`)
   - Platform-specific kernel-enforced restrictions
   - Process isolation and system call filtering
   - Mandatory enforcement regardless of plugin cooperation

## Platform Support

### Windows (`os_sandbox` feature)
- **Technology**: Job Objects with process limits
- **Minimal Policy**: Prevents child process creation, ensures cleanup on termination
- **Strict Policy**: Same as minimal (future expansion planned)
- **Dependencies**: `win32job` crate for safe Job Object management

### Linux (`os_sandbox` feature)
- **Technology**: seccomp-bpf system call filtering
- **Minimal Policy**: Blocks dangerous syscalls (fork, exec, ptrace, mount operations)
- **Strict Policy**: Additional restrictions on network and filesystem syscalls
- **Dependencies**: `seccompiler` and `seccomp` crates for pure-Rust implementation

### macOS (`os_sandbox` feature)
- **Technology**: sandbox_init with custom profiles
- **Minimal Policy**: Allows most operations, blocks process creation/execution
- **Strict Policy**: Minimal filesystem access, no network operations
- **Dependencies**: System sandbox framework (no additional crates)

### OpenBSD (`os_sandbox` feature)
- **Technology**: pledge/unveil system
- **Minimal Policy**: stdio, rpath, wpath, cpath, inet, unix, proc promises
- **Strict Policy**: stdio, rpath promises only
- **Dependencies**: `pledge` and `unveil` crates

### Other Platforms
- **Status**: `SandboxStatus::Unsupported`
- **Behavior**: Application-level guards still function, no OS enforcement

## Configuration

### Enabling OS Sandbox
```toml
[dependencies.nyx-core]
features = ["os_sandbox"]
```

### Usage Example
```rust
use nyx_core::sandbox::{apply_policy, SandboxPolicy, SandboxStatus};

// Apply minimal restrictions
let status = apply_policy(SandboxPolicy::Minimal);
match status {
    SandboxStatus::Applied => println!("Sandbox active"),
    SandboxStatus::Unsupported => println!("OS sandbox not available"),
}

// Apply strict restrictions
apply_policy(SandboxPolicy::Strict);
```

### Application-Level Policy
```rust
use nyx_stream::plugin_sandbox::{SandboxGuard, SandboxPolicy};

let policy = SandboxPolicy::default()
    .allow_connect_host("api.example.com")
    .allow_path_prefix("/var/lib/nyx");

let guard = SandboxGuard::new(policy);

// Check operations before execution
if guard.check_connect("api.example.com:443").is_ok() {
    // Proceed with connection
}
```

## Security Considerations

### Defense in Depth
- Both layers should be enabled for maximum security
- Application-level guards catch policy violations early
- OS-level restrictions provide mandatory enforcement

### Platform Differences
- Windows: Job Objects provide process lifecycle management
- Linux: seccomp filters offer fine-grained syscall control  
- macOS: Sandbox profiles provide comprehensive access control
- OpenBSD: pledge/unveil offers capability-based security

### Limitations
- Sandbox policies are applied at process startup
- Some platforms may not support all restriction types
- Plugin cooperation required for application-level enforcement

## Testing

### Unit Tests
- Cross-platform policy application testing
- Application-level guard validation
- Error handling for unsupported platforms

### Integration Tests
- End-to-end plugin execution with sandbox
- Policy violation detection and blocking
- Platform-specific restriction verification

## Future Enhancements

1. **Dynamic Policy Updates**: Support for runtime policy modification
2. **Resource Limits**: Memory and CPU usage restrictions
3. **Extended Platform Support**: Additional OS-specific implementations
4. **Audit Logging**: Detailed sandbox violation reporting
5. **Performance Optimization**: Reduced overhead for policy checks
