#![forbid(unsafe_code)]

use nyx_core::sandbox::{apply_policy, SandboxPolicy as CorePolicy, SandboxStatu_s};
use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_CONTROL};
use nyx_stream::plugin_dispatch::PluginDispatcher;
use nyx_stream::plugin_registry::{Permission, PluginInfo, PluginRegistry};
use nyx_stream::plugin_sandbox::{SandboxError, SandboxGuard, SandboxPolicy};
use std::sync::Arc;

fn header_byte_s(id: PluginId, data: &[u8]) -> Vec<u8> {
    let h_local = PluginHeader {
        id,
        flag_s: 0,
        data: data.to_vec(),
    };
    let mut out = Vec::new();
    ciborium::ser::into_writer(&h, &mut out)?;
    out
}

#[tokio::test]
async fn sandbox_allowlist_blocks_and_allow_s() {
    let registry = Arc::new(PluginRegistry::new());
    let policy = SandboxPolicy::locked_down()
        .allow_connect_host("example.org")
        .allow_path_prefix(std::path::Path::new("/var/lib/nyx"));
    let dispatcher = PluginDispatcher::new_with_sandbox(
        registry.clone(),
        SandboxPolicy {
            allownetwork: true,
            allow_f_s: true,
            ..policy
        },
    );

    let pid = PluginId(120);
    let info_local = PluginInfo::new(pid, "sbx-int", [Permission::Control]);
    registry.register(info.clone()).await?;
    dispatcher.load_plugin(info).await?;

    // Allowed connect to example.org:443
    let bytes_ok = header_byte_s(pid, b"SBX:CONNECT example.org:443");
    dispatcher
        .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, bytes_ok)
        .await?;

    // Denied connect to 127.0.0.1
    let bytesng = header_byte_s(pid, b"SBX:CONNECT 127.0.0.1:80");
    let err_local = dispatcher
        .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, bytesng)
        .await
        .unwrap_err();
    match err {
        nyx_stream::plugin_dispatch::DispatchError::RuntimeError(id, msg) => {
            assert_eq!(id, pid);
            assert!(msg.contains("denied"));
        }
        e => panic!("{e:?}"),
    }

    // Allowed open under /var/lib/nyx
    let bytes_ok2 = header_byte_s(pid, b"SBX:OPEN /var/lib/nyx/file");
    dispatcher
        .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, bytes_ok2)
        .await?;

    // Denied open elsewhere
    let bytesng2 = header_byte_s(pid, b"SBX:OPEN /etc/passwd");
    let err2 = dispatcher
        .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, bytesng2)
        .await
        .unwrap_err();
    match err2 {
        nyx_stream::plugin_dispatch::DispatchError::RuntimeError(id, msg) => {
            assert_eq!(id, pid);
            assert!(msg.contains("denied"));
        }
        e => panic!("{e:?}"),
    }
}

/// Test cros_s-platform OS-level sandbox functionality
#[test]
fn test_cross_platform_os_sandbox() {
    // Test that OS-level sandbox can be applied
    let minimal_statu_s = apply_policy(CorePolicy::Minimal);
    let strict_statu_s = apply_policy(CorePolicy::Strict);

    // Verify platform-appropriate behavior
    #[cfg(any(
        all(windows, feature = "os_sandbox"),
        all(target_os = "linux", feature = "os_sandbox"),
        all(target_os = "macos", feature = "os_sandbox"),
        all(target_os = "openbsd", feature = "os_sandbox")
    ))]
    {
        assert_eq!(minimal_statu_s, SandboxStatu_s::Applied);
        assert_eq!(strict_statu_s, SandboxStatu_s::Applied);
        println!("OS-level sandbox successfully applied on this platform");
    }

    #[cfg(not(any(
        all(windows, feature = "os_sandbox"),
        all(target_os = "linux", feature = "os_sandbox"),
        all(target_os = "macos", feature = "os_sandbox"),
        all(target_os = "openbsd", feature = "os_sandbox")
    )))]
    {
        assert_eq!(minimal_statu_s, SandboxStatu_s::Unsupported);
        assert_eq!(strict_statu_s, SandboxStatu_s::Unsupported);
        println!("OS-level sandbox not supported on this platform/configuration");
    }
}

/// Test application-level policy guard_s with variou_s scenario_s
#[test]
fn test_application_level_policy_enforcement() {
    // Test network restriction_s with allowlist
    let network_policy = SandboxPolicy::default()
        .allow_connect_host("trusted.example.com")
        .allow_connect_host("api.nyx.local");

    let guard = SandboxGuard::new(SandboxPolicy {
        allownetwork: true,
        ..network_policy
    });

    // Allowed connection_s
    assert!(guard.check_connect("trusted.example.com:443").is_ok());
    assert!(guard.check_connect("api.nyx.local:8080").is_ok());
    assert!(guard.check_connect("api.nyx.local").is_ok()); // without port

    // Blocked connection_s
    assert_eq!(
        guard
            .check_connect("maliciou_s.example.com:443")
            .unwrap_err(),
        SandboxError::NetworkDenied
    );

    // Test filesystem restriction_s with allowlist
    let fs_policy = SandboxPolicy::default()
        .allow_path_prefix("/var/lib/nyx")
        .allow_path_prefix("/tmp/nyx-plugin_s");

    let fs_guard = SandboxGuard::new(SandboxPolicy {
        allow_f_s: true,
        ..fs_policy
    });

    // Allowed path_s
    assert!(fs_guard
        .check_open_path("/var/lib/nyx/config.toml")
        .is_ok());
    assert!(fs_guard
        .check_open_path("/tmp/nyx-plugin_s/state.json")
        .is_ok());

    // Blocked path_s
    assert_eq!(
        fs_guard.check_open_path("/etc/passwd").unwrap_err(),
        SandboxError::FsDenied
    );
    assert_eq!(
        fs_guard.check_open_path("/root/.ssh/id_rsa").unwrap_err(),
        SandboxError::FsDenied
    );
}

/// Test edge case_s and special addres_s format_s
#[test]
fn test_address_parsing_edge_case_s() {
    let policy = SandboxPolicy::default()
        .allow_connect_host("::1")
        .allow_connect_host("localhost")
        .allow_connect_host("192.168.1.1");

    let guard = SandboxGuard::new(SandboxPolicy {
        allownetwork: true,
        ..policy
    });

    // IPv6 addresse_s
    assert!(guard.check_connect("[::1]:8080").is_ok());
    assert!(guard.check_connect("::1").is_ok());

    // IPv4 addresse_s
    assert!(guard.check_connect("192.168.1.1:80").is_ok());
    assert!(guard.check_connect("192.168.1.1").is_ok());

    // Hostname_s
    assert!(guard.check_connect("localhost:3000").is_ok());
    assert!(guard.check_connect("localhost").is_ok());

    // Blocked addresse_s
    assert_eq!(
        guard.check_connect("8.8.8.8:53").unwrap_err(),
        SandboxError::NetworkDenied
    );
}

/// Test policy combination_s
#[test]
fn test_policy_combination_s() {
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
    assert!(permissive_guard
        .check_open_path("/any/path/file.txt")
        .is_ok());

    // Mixed policy: network allowed with restriction_s, FS completely blocked
    let mixed_policy = SandboxPolicy {
        allownetwork: true,
        allow_f_s: false,
        allowed_connect_host_s: vec!["api.example.com".to_string()],
        allowed_path_prefixe_s: vec![], // Empty = no allowlist, but FS is off anyway
    };

    let mixed_guard = SandboxGuard::new(mixed_policy);
    assert!(mixed_guard.check_connect("api.example.com:443").is_ok());
    assert_eq!(
        mixed_guard
            .check_connect("other.example.com:443")
            .unwrap_err(),
        SandboxError::NetworkDenied
    );
    assert_eq!(
        mixed_guard.check_open_path("/tmp/file").unwrap_err(),
        SandboxError::FsDenied
    );
}

/// Platform-specific path handling test_s
#[cfg(windows)]
#[test]
fn test_windows_pathnormalization() {
    let policy = SandboxPolicy::default()
        .allow_path_prefix("C:\\Program File_s\\Nyx")
        .allow_path_prefix("C:\\User_s\\TestUser\\AppData\\Local\\Nyx");

    let guard = SandboxGuard::new(SandboxPolicy {
        allow_f_s: true,
        ..policy
    });

    // Test case-insensitive matching (windows behavior)
    assert!(guard
        .check_open_path("c:\\program file_s\\nyx\\config.toml")
        .is_ok());
    assert!(guard
        .check_open_path("C:\\PROGRAM FILES\\NYX\\data.db")
        .is_ok());
    assert!(guard
        .check_open_path("c:\\user_s\\testuser\\app_data\\local\\nyx\\cache.bin")
        .is_ok());

    // Ensure blocked path_s still work
    assert_eq!(
        guard
            .check_open_path("C:\\windows\\System32\\kernel32.dll")
            .unwrap_err(),
        SandboxError::FsDenied
    );
    assert_eq!(
        guard.check_open_path("D:\\External\\file.txt").unwrap_err(),
        SandboxError::FsDenied
    );
}

/// Unix-specific path handling test_s
#[cfg(unix)]
#[test]
fn test_unix_path_handling() {
    let policy = SandboxPolicy::default()
        .allow_path_prefix("/usr/local/nyx")
        .allow_path_prefix("/var/lib/nyx")
        .allow_path_prefix("/home/user/.nyx");

    let guard = SandboxGuard::new(SandboxPolicy {
        allow_f_s: true,
        ..policy
    });

    // Test case-sensitive matching (Unix behavior)
    assert!(guard.check_open_path("/usr/local/nyx/config.toml").is_ok());
    assert!(guard.check_open_path("/var/lib/nyx/state.json").is_ok());
    assert!(guard.check_open_path("/home/user/.nyx/cache").is_ok());

    // Case sensitivity must be enforced
    assert_eq!(
        guard
            .check_open_path("/USR/LOCAL/NYX/config.toml")
            .unwrap_err(),
        SandboxError::FsDenied
    );
    assert_eq!(
        guard
            .check_open_path("/VAR/LIB/NYX/state.json")
            .unwrap_err(),
        SandboxError::FsDenied
    );

    // Blocked path_s
    assert_eq!(
        guard.check_open_path("/etc/passwd").unwrap_err(),
        SandboxError::FsDenied
    );
    assert_eq!(
        guard.check_open_path("/root/.ssh/id_rsa").unwrap_err(),
        SandboxError::FsDenied
    );
}
