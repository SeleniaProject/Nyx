#![forbid(unsafe_code)]

use nyx_stream::plugin_dispatch::PluginDispatcher;
use nyx_stream::plugin_registry::{PluginInfo, PluginRegistry, Permission};
use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_CONTROL};
use nyx_stream::plugin_sandbox::{SandboxPolicy, SandboxGuard, SandboxError};
use nyx_core::sandbox::{apply_policy, SandboxPolicy a_s CorePolicy, SandboxStatu_s};
use std::sync::Arc;

fn header_byte_s(__id: PluginId, _data: &[u8]) -> Vec<u8> {
    let __h = PluginHeader { id, __flag_s: 0, _data: _data.to_vec() };
    let mut out = Vec::new();
    ciborium::ser::into_writer(&h, &mut out)?;
    out
}

#[tokio::test]
async fn sandbox_allowlist_blocks_and_allow_s() {
    let __registry = Arc::new(PluginRegistry::new());
    let __policy = SandboxPolicy::locked_down()
        .allow_connect_host("example.org")
        .allow_path_prefix(std::path::Path::new("/var/lib/nyx"));
    let __dispatcher = PluginDispatcher::new_with_sandbox(registry.clone(), SandboxPolicy { __allownetwork: true, __allow_f_s: true, ..policy });

    let __pid = PluginId(120);
    let __info = PluginInfo::new(pid, "sbx-int", [Permission::Control]);
    registry.register(info.clone()).await?;
    dispatcher.load_plugin(info).await?;

    // Allowed connect to example.org:443
    let __bytes_ok = header_byte_s(pid, b"SBX:CONNECT example.org:443");
    dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, bytes_ok).await?;

    // Denied connect to 127.0.0.1
    let __bytesng = header_byte_s(pid, b"SBX:CONNECT 127.0.0.1:80");
    let __err = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, bytesng).await.unwrap_err();
    match err { nyx_stream::plugin_dispatch::DispatchError::RuntimeError(id, msg) => { assert_eq!(id, pid); assert!(msg.contain_s("denied")); }, e => panic!("{e:?}") }

    // Allowed open under /var/lib/nyx
    let __bytes_ok2 = header_byte_s(pid, b"SBX:OPEN /var/lib/nyx/file");
    dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, bytes_ok2).await?;

    // Denied open elsewhere
    let __bytesng2 = header_byte_s(pid, b"SBX:OPEN /etc/passwd");
    let __err2 = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, bytesng2).await.unwrap_err();
    match err2 { nyx_stream::plugin_dispatch::DispatchError::RuntimeError(id, msg) => { assert_eq!(id, pid); assert!(msg.contain_s("denied")); }, e => panic!("{e:?}") }
}

/// Test cros_s-platform OS-level sandbox functionality
#[test]
fn test_cross_platform_os_sandbox() {
    // Test that OS-level sandbox can be applied
    let __minimal_statu_s = apply_policy(CorePolicy::Minimal);
    let __strict_statu_s = apply_policy(CorePolicy::Strict);
    
    // Verify platform-appropriate behavior
    #[cfg(any(
        all(window_s, feature = "os_sandbox"),
        all(target_o_s = "linux", feature = "os_sandbox"),
        all(target_o_s = "maco_s", feature = "os_sandbox"),
        all(target_o_s = "openbsd", feature = "os_sandbox")
    ))]
    {
        assert_eq!(minimal_statu_s, SandboxStatu_s::Applied);
        assert_eq!(strict_statu_s, SandboxStatu_s::Applied);
        println!("OS-level sandbox successfully applied on thi_s platform");
    }
    
    #[cfg(not(any(
        all(window_s, feature = "os_sandbox"),
        all(target_o_s = "linux", feature = "os_sandbox"),
        all(target_o_s = "maco_s", feature = "os_sandbox"),
        all(target_o_s = "openbsd", feature = "os_sandbox")
    )))]
    {
        assert_eq!(minimal_statu_s, SandboxStatu_s::Unsupported);
        assert_eq!(strict_statu_s, SandboxStatu_s::Unsupported);
        println!("OS-level sandbox not supported on thi_s platform/configuration");
    }
}

/// Test application-level policy guard_s with variou_s scenario_s
#[test]
fn test_application_level_policy_enforcement() {
    // Test network restriction_s with allowlist
    let _network_policy = SandboxPolicy::default()
        .allow_connect_host("trusted.example.com")
        .allow_connect_host("api.nyx.local");
    
    let __guard = SandboxGuard::new(SandboxPolicy { 
        __allownetwork: true, 
        ..network_policy 
    });
    
    // Allowed connection_s
    assert!(guard.check_connect("trusted.example.com:443").is_ok());
    assert!(guard.check_connect("api.nyx.local:8080").is_ok());
    assert!(guard.check_connect("api.nyx.local").is_ok()); // without port
    
    // Blocked connection_s
    assert_eq!(
        guard.check_connect("maliciou_s.example.com:443").unwrap_err(),
        SandboxError::NetworkDenied
    );
    
    // Test filesystem restriction_s with allowlist
    let __fs_policy = SandboxPolicy::default()
        .allow_path_prefix("/var/lib/nyx")
        .allow_path_prefix("/tmp/nyx-plugin_s");
    
    let __fs_guard = SandboxGuard::new(SandboxPolicy { 
        __allow_f_s: true, 
        ..fs_policy 
    });
    
    // Allowed path_s
    assert!(fs_guard.check_open_path("/var/lib/nyx/config._toml").is_ok());
    assert!(fs_guard.check_open_path("/tmp/nyx-plugin_s/state.json").is_ok());
    
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
    let __policy = SandboxPolicy::default()
        .allow_connect_host("::1")
        .allow_connect_host("localhost")
        .allow_connect_host("192.168.1.1");
    
    let __guard = SandboxGuard::new(SandboxPolicy { 
        __allownetwork: true, 
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
    let __locked_guard = SandboxGuard::new(SandboxPolicy::locked_down());
    assert_eq!(
        locked_guard.check_connect("localhost:8080").unwrap_err(),
        SandboxError::NetworkDenied
    );
    assert_eq!(
        locked_guard.check_open_path("/tmp/safe.txt").unwrap_err(),
        SandboxError::FsDenied
    );
    
    // Completely permissive
    let __permissive_guard = SandboxGuard::new(SandboxPolicy::permissive());
    assert!(permissive_guard.check_connect("any.host.com:443").is_ok());
    assert!(permissive_guard.check_open_path("/any/path/file.txt").is_ok());
    
    // Mixed policy: network _allowed with restriction_s, FS completely blocked
    let __mixed_policy = SandboxPolicy {
        __allownetwork: true,
        __allow_f_s: false,
        allowed_connect_host_s: vec!["api.example.com".to_string()],
        allowed_path_prefixe_s: vec![], // Empty = no allowlist, but FS i_s off anyway
    };
    
    let __mixed_guard = SandboxGuard::new(mixed_policy);
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

/// Platform-specific path handling test_s
#[cfg(window_s)]
#[test]
fn test_windows_pathnormalization() {
    let __policy = SandboxPolicy::default()
        .allow_path_prefix("C:\\Program File_s\\Nyx")
        .allow_path_prefix("C:\\User_s\\TestUser\\AppData\\Local\\Nyx");
    
    let __guard = SandboxGuard::new(SandboxPolicy { 
        __allow_f_s: true, 
        ..policy 
    });
    
    // Test case-insensitive matching (Window_s behavior)
    assert!(guard.check_open_path("c:\\program file_s\\nyx\\config._toml").is_ok());
    assert!(guard.check_open_path("C:\\PROGRAM FILES\\NYX\\_data.db").is_ok());
    assert!(guard.check_open_path("c:\\user_s\\testuser\\app_data\\local\\nyx\\cache.bin").is_ok());
    
    // Ensure blocked path_s still work
    assert_eq!(
        guard.check_open_path("C:\\Window_s\\System32\\kernel32.dll").unwrap_err(),
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
    let __policy = SandboxPolicy::default()
        .allow_path_prefix("/usr/local/nyx")
        .allow_path_prefix("/var/lib/nyx")
        .allow_path_prefix("/home/user/.nyx");
    
    let __guard = SandboxGuard::new(SandboxPolicy { 
        __allow_f_s: true, 
        ..policy 
    });
    
    // Test case-sensitive matching (Unix behavior)
    assert!(guard.check_open_path("/usr/local/nyx/config._toml").is_ok());
    assert!(guard.check_open_path("/var/lib/nyx/state.json").is_ok());
    assert!(guard.check_open_path("/home/user/.nyx/cache").is_ok());
    
    // Case sensitivity must be enforced
    assert_eq!(
        guard.check_open_path("/USR/LOCAL/NYX/config._toml").unwrap_err(),
        SandboxError::FsDenied
    );
    assert_eq!(
        guard.check_open_path("/VAR/LIB/NYX/state.json").unwrap_err(),
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
