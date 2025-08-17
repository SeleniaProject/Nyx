#![forbid(unsafe_code)]

use nyx_stream::plugin_dispatch::PluginDispatcher;
use nyx_stream::plugin_registry::{PluginInfo, PluginRegistry, Permission};
use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_CONTROL};
use nyx_stream::plugin_sandbox::{SandboxPolicy, SandboxGuard, SandboxError};
use nyx_core::sandbox::{apply_policy, SandboxPolicy as CorePolicy, SandboxStatus};
use std::sync::Arc;

fn header_bytes(id: PluginId, data: &[u8]) -> Vec<u8> {
    let h = PluginHeader { id, flags: 0, data: data.to_vec() };
    let mut out = Vec::new();
    ciborium::ser::into_writer(&h, &mut out).expect("serialize header");
    out
}

#[tokio::test]
async fn sandbox_allowlist_blocks_and_allows() {
    let registry = Arc::new(PluginRegistry::new());
    let policy = SandboxPolicy::locked_down()
        .allow_connect_host("example.org")
        .allow_path_prefix(std::path::Path::new("/var/lib/nyx"));
    let dispatcher = PluginDispatcher::new_with_sandbox(registry.clone(), SandboxPolicy { allow_network: true, allow_fs: true, ..policy });

    let pid = PluginId(120);
    let info = PluginInfo::new(pid, "sbx-int", [Permission::Control]);
    registry.register(info.clone()).await.unwrap();
    dispatcher.load_plugin(info).await.unwrap();

    // Allowed connect to example.org:443
    let bytes_ok = header_bytes(pid, b"SBX:CONNECT example.org:443");
    dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, bytes_ok).await.unwrap();

    // Denied connect to 127.0.0.1
    let bytes_ng = header_bytes(pid, b"SBX:CONNECT 127.0.0.1:80");
    let err = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, bytes_ng).await.unwrap_err();
    match err { nyx_stream::plugin_dispatch::DispatchError::RuntimeError(id, msg) => { assert_eq!(id, pid); assert!(msg.contains("denied")); }, e => panic!("{e:?}") }

    // Allowed open under /var/lib/nyx
    let bytes_ok2 = header_bytes(pid, b"SBX:OPEN /var/lib/nyx/file");
    dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, bytes_ok2).await.unwrap();

    // Denied open elsewhere
    let bytes_ng2 = header_bytes(pid, b"SBX:OPEN /etc/passwd");
    let err2 = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, bytes_ng2).await.unwrap_err();
    match err2 { nyx_stream::plugin_dispatch::DispatchError::RuntimeError(id, msg) => { assert_eq!(id, pid); assert!(msg.contains("denied")); }, e => panic!("{e:?}") }
}

/// Test cross-platform OS-level sandbox functionality
#[test]
fn test_cross_platform_os_sandbox() {
    // Test that OS-level sandbox can be applied
    let minimal_status = apply_policy(CorePolicy::Minimal);
    let strict_status = apply_policy(CorePolicy::Strict);
    
    // Verify platform-appropriate behavior
    #[cfg(any(
        all(windows, feature = "os_sandbox"),
        all(target_os = "linux", feature = "os_sandbox"),
        all(target_os = "macos", feature = "os_sandbox"),
        all(target_os = "openbsd", feature = "os_sandbox")
    ))]
    {
        assert_eq!(minimal_status, SandboxStatus::Applied);
        assert_eq!(strict_status, SandboxStatus::Applied);
        println!("OS-level sandbox successfully applied on this platform");
    }
    
    #[cfg(not(any(
        all(windows, feature = "os_sandbox"),
        all(target_os = "linux", feature = "os_sandbox"),
        all(target_os = "macos", feature = "os_sandbox"),
        all(target_os = "openbsd", feature = "os_sandbox")
    )))]
    {
        assert_eq!(minimal_status, SandboxStatus::Unsupported);
        assert_eq!(strict_status, SandboxStatus::Unsupported);
        println!("OS-level sandbox not supported on this platform/configuration");
    }
}

/// Test application-level policy guards with various scenarios
#[test]
fn test_application_level_policy_enforcement() {
    // Test network restrictions with allowlist
    let network_policy = SandboxPolicy::default()
        .allow_connect_host("trusted.example.com")
        .allow_connect_host("api.nyx.local");
    
    let guard = SandboxGuard::new(SandboxPolicy { 
        allow_network: true, 
        ..network_policy 
    });
    
    // Allowed connections
    assert!(guard.check_connect("trusted.example.com:443").is_ok());
    assert!(guard.check_connect("api.nyx.local:8080").is_ok());
    assert!(guard.check_connect("api.nyx.local").is_ok()); // without port
    
    // Blocked connections
    assert_eq!(
        guard.check_connect("malicious.example.com:443").unwrap_err(),
        SandboxError::NetworkDenied
    );
    
    // Test filesystem restrictions with allowlist
    let fs_policy = SandboxPolicy::default()
        .allow_path_prefix("/var/lib/nyx")
        .allow_path_prefix("/tmp/nyx-plugins");
    
    let fs_guard = SandboxGuard::new(SandboxPolicy { 
        allow_fs: true, 
        ..fs_policy 
    });
    
    // Allowed paths
    assert!(fs_guard.check_open_path("/var/lib/nyx/config.toml").is_ok());
    assert!(fs_guard.check_open_path("/tmp/nyx-plugins/state.json").is_ok());
    
    // Blocked paths
    assert_eq!(
        fs_guard.check_open_path("/etc/passwd").unwrap_err(),
        SandboxError::FsDenied
    );
    assert_eq!(
        fs_guard.check_open_path("/root/.ssh/id_rsa").unwrap_err(),
        SandboxError::FsDenied
    );
}

/// Test edge cases and special address formats
#[test]
fn test_address_parsing_edge_cases() {
    let policy = SandboxPolicy::default()
        .allow_connect_host("::1")
        .allow_connect_host("localhost")
        .allow_connect_host("192.168.1.1");
    
    let guard = SandboxGuard::new(SandboxPolicy { 
        allow_network: true, 
        ..policy 
    });
    
    // IPv6 addresses
    assert!(guard.check_connect("[::1]:8080").is_ok());
    assert!(guard.check_connect("::1").is_ok());
    
    // IPv4 addresses
    assert!(guard.check_connect("192.168.1.1:80").is_ok());
    assert!(guard.check_connect("192.168.1.1").is_ok());
    
    // Hostnames
    assert!(guard.check_connect("localhost:3000").is_ok());
    assert!(guard.check_connect("localhost").is_ok());
    
    // Blocked addresses
    assert_eq!(
        guard.check_connect("8.8.8.8:53").unwrap_err(),
        SandboxError::NetworkDenied
    );
}

/// Test policy combinations
#[test]
fn test_policy_combinations() {
    // Completely locked down
    let locked_guard = SandboxGuard::new(SandboxPolicy::locked_down());
    assert_eq!(
        locked_guard.check_connect("localhost:8080").unwrap_err(),
        SandboxError::NetworkDenied
    );
    assert_eq!(
        locked_guard.check_open_path("/tmp/safe.txt").unwrap_err(),
        SandboxError::FsDenied
    );
    
    // Completely permissive
    let permissive_guard = SandboxGuard::new(SandboxPolicy::permissive());
    assert!(permissive_guard.check_connect("any.host.com:443").is_ok());
    assert!(permissive_guard.check_open_path("/any/path/file.txt").is_ok());
    
    // Mixed policy: network allowed with restrictions, FS completely blocked
    let mixed_policy = SandboxPolicy {
        allow_network: true,
        allow_fs: false,
        allowed_connect_hosts: vec!["api.example.com".to_string()],
        allowed_path_prefixes: vec![], // Empty = no allowlist, but FS is off anyway
    };
    
    let mixed_guard = SandboxGuard::new(mixed_policy);
    assert!(mixed_guard.check_connect("api.example.com:443").is_ok());
    assert_eq!(
        mixed_guard.check_connect("other.example.com:443").unwrap_err(),
        SandboxError::NetworkDenied
    );
    assert_eq!(
        mixed_guard.check_open_path("/tmp/file").unwrap_err(),
        SandboxError::FsDenied
    );
}

/// Platform-specific path handling tests
#[cfg(windows)]
#[test]
fn test_windows_path_normalization() {
    let policy = SandboxPolicy::default()
        .allow_path_prefix("C:\\Program Files\\Nyx")
        .allow_path_prefix("C:\\Users\\TestUser\\AppData\\Local\\Nyx");
    
    let guard = SandboxGuard::new(SandboxPolicy { 
        allow_fs: true, 
        ..policy 
    });
    
    // Test case-insensitive matching (Windows behavior)
    assert!(guard.check_open_path("c:\\program files\\nyx\\config.toml").is_ok());
    assert!(guard.check_open_path("C:\\PROGRAM FILES\\NYX\\data.db").is_ok());
    assert!(guard.check_open_path("c:\\users\\testuser\\appdata\\local\\nyx\\cache.bin").is_ok());
    
    // Ensure blocked paths still work
    assert_eq!(
        guard.check_open_path("C:\\Windows\\System32\\kernel32.dll").unwrap_err(),
        SandboxError::FsDenied
    );
    assert_eq!(
        guard.check_open_path("D:\\External\\file.txt").unwrap_err(),
        SandboxError::FsDenied
    );
}

/// Unix-specific path handling tests
#[cfg(unix)]
#[test]
fn test_unix_path_handling() {
    let policy = SandboxPolicy::default()
        .allow_path_prefix("/usr/local/nyx")
        .allow_path_prefix("/var/lib/nyx")
        .allow_path_prefix("/home/user/.nyx");
    
    let guard = SandboxGuard::new(SandboxPolicy { 
        allow_fs: true, 
        ..policy 
    });
    
    // Test case-sensitive matching (Unix behavior)
    assert!(guard.check_open_path("/usr/local/nyx/config.toml").is_ok());
    assert!(guard.check_open_path("/var/lib/nyx/state.json").is_ok());
    assert!(guard.check_open_path("/home/user/.nyx/cache").is_ok());
    
    // Case sensitivity must be enforced
    assert_eq!(
        guard.check_open_path("/USR/LOCAL/NYX/config.toml").unwrap_err(),
        SandboxError::FsDenied
    );
    assert_eq!(
        guard.check_open_path("/VAR/LIB/NYX/state.json").unwrap_err(),
        SandboxError::FsDenied
    );
    
    // Blocked paths
    assert_eq!(
        guard.check_open_path("/etc/passwd").unwrap_err(),
        SandboxError::FsDenied
    );
    assert_eq!(
        guard.check_open_path("/root/.ssh/id_rsa").unwrap_err(),
        SandboxError::FsDenied
    );
}
