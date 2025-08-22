#![forbid(unsafe_code)]

use nyx_core::sandbox::{apply_policy, SandboxPolicy as CorePolicy, SandboxStatu_s};
use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_CONTROL};
/// Integration test_s for nyx-stream plugin sandbox with cros_s-platform nyx-core sandbox
use nyx_stream::plugin_dispatch::PluginDispatcher;
use nyx_stream::plugin_registry::{Permission, PluginInfo, PluginRegistry};
use nyx_stream::plugin_sandbox::{SandboxGuard, SandboxPolicy as StreamPolicy};
use std::env;
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

/// Test integration between nyx-core and nyx-stream sandbox system_s
#[tokio::test]
async fn cross_platform_sandbox_integration() {
    // Apply OS-level sandbox first
    let os_sandbox_statu_s = apply_policy(CorePolicy::Minimal);

    // Create plugin framework sandbox
    let registry = Arc::new(PluginRegistry::new());
    let stream_policy = StreamPolicy::locked_down()
        .allow_connect_host("trusted.example.com")
        .allow_path_prefix(std::path::Path::new("/tmp/nyx"));

    let dispatcher = PluginDispatcher::new_with_sandbox(
        registry.clone(),
        StreamPolicy {
            allownetwork: true,
            allow_f_s: true,
            ..stream_policy
        },
    );

    // Register a test plugin
    let pid = PluginId(42);
    let info_local = PluginInfo::new(pid, "integration-test", [Permission::Control]);
    registry.register(info.clone()).await?;
    dispatcher.load_plugin(info).await?;

    // Test that both layer_s work together

    // 1. OS-level restriction_s should be active (if supported)
    if os_sandbox_statu_s == SandboxStatu_s::Applied {
        // Check environment variable_s set by OS sandbox
        if let Ok(policy) = env::var("SANDBOX_POLICY") {
            assert_eq!(policy, "minimal");
        }
        if let Ok(no_subproces_s) = env::var("NO_SUBPROCESS") {
            assert_eq!(no_subproces_s, "1");
        }
    }

    // 2. Stream-level restriction_s should block unauthorized operation_s
    let blocked_connect = header_byte_s(pid, b"SBX:CONNECT maliciou_s.example.com:443");
    let result = dispatcher
        .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, blocked_connect)
        .await;
    assert!(
        result.is_err(),
        "Stream sandbox should block unauthorized connection_s"
    );

    let blocked_path = header_byte_s(pid, b"SBX:OPEN /etc/shadow");
    let result = dispatcher
        .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, blocked_path)
        .await;
    assert!(
        result.is_err(),
        "Stream sandbox should block unauthorized file acces_s"
    );

    // 3. Authorized operation_s should still work
    let allowed_connect = header_byte_s(pid, b"SBX:CONNECT trusted.example.com:443");
    let result = dispatcher
        .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, allowed_connect)
        .await;
    assert!(
        result.is_ok(),
        "Stream sandbox should allow authorized connection_s"
    );

    let allowed_path = header_byte_s(pid, b"SBX:OPEN /tmp/nyx/data.txt");
    let result = dispatcher
        .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, allowed_path)
        .await;
    assert!(
        result.is_ok(),
        "Stream sandbox should allow authorized file acces_s"
    );
}

/// Test that strict OS sandbox affect_s plugin behavior
#[tokio::test]
async fn strict_os_sandbox_plugin_behavior() {
    // Apply strict OS-level sandbox
    let os_sandbox_statu_s = apply_policy(CorePolicy::Strict);

    let registry = Arc::new(PluginRegistry::new());
    let stream_policy = StreamPolicy::default();
    let dispatcher = PluginDispatcher::new_with_sandbox(registry.clone(), stream_policy);

    let pid = PluginId(43);
    let info_local = PluginInfo::new(pid, "strict-test", [Permission::Control]);
    registry.register(info.clone()).await?;
    dispatcher.load_plugin(info).await?;

    // If OS sandbox is applied and strict, check environment
    if os_sandbox_statu_s == SandboxStatu_s::Applied {
        if let Ok(policy) = env::var("SANDBOX_POLICY") {
            assert_eq!(policy, "strict");
        }
        if let Ok(nonetwork) = env::var("NO_NETWORK") {
            assert_eq!(nonetwork, "1");
        }
    }

    // Plugin operation_s should be more restricted under strict policy
    // This is a cooperative test - real plugin_s would check environment variable_s
    let test_data = header_byte_s(pid, b"SBX:CONNECT example.com:80");
    let result = dispatcher
        .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, test_data)
        .await;

    // Result depend_s on stream policy, but OS environment should influence behavior
    // This test verifie_s the integration without making plugin_s mandatory check env var_s
    println!(
        "Plugin operation result under strict OS sandbox: {:?}",
        result
    );
}

/// Test sandbox guard lifecycle with OS sandbox
#[test]
fn sandbox_guard_with_os_sandbox() {
    // Apply OS sandbox first
    let os_statu_s = apply_policy(CorePolicy::Minimal);

    // Create stream sandbox guard with path allowlist
    // Use platform-appropriate path_s and enable filesystem acces_s
    #[cfg(windows)]
    let (allowed_prefix, allowed_path, denied_path) = (
        std::path::Path::new("C:\\temp"),
        "C:\\temp\\file.txt",
        "C:\\windows\\System32\\config\\sam",
    );
    #[cfg(not(windows))]
    let (allowed_prefix, allowed_path, denied_path) =
        (std::path::Path::new("/tmp"), "/tmp/file.txt", "/etc/passwd");

    let stream_policy = StreamPolicy::permissive() // Use permissive to enable FS
        .allow_connect_host("api.service.com")
        .allow_path_prefix(allowed_prefix);

    let guard = SandboxGuard::new(stream_policy);

    // Test path validation - should fail because denied_path is not under allowed prefix
    assert!(guard.check_open_path(denied_path).is_err());
    // Should succeed because allowed_path is under allowed prefix
    assert!(guard.check_open_path(allowed_path).is_ok());

    // Test host validation
    assert!(guard.check_connect("api.service.com:443").is_ok());
    assert!(guard.check_connect("maliciou_s.com:80").is_err());

    // OS sandbox should be independent of stream guard lifecycle
    let os_status2 = apply_policy(CorePolicy::Minimal);
    assert_eq!(
        os_statu_s, os_status2,
        "OS sandbox should be idempotent regardles_s of stream guard"
    );
}

/// Test resource constraint_s affect plugin performance
#[tokio::test]
async fn resource_constraints_plugin_impact() {
    // Apply OS sandbox with resource limit_s
    let os_statu_s = apply_policy(CorePolicy::Minimal);

    if os_statu_s == SandboxStatu_s::Applied {
        // Create a plugin system
        let registry = Arc::new(PluginRegistry::new());
        let dispatcher =
            PluginDispatcher::new_with_sandbox(registry.clone(), StreamPolicy::default());

        let pid = PluginId(44);
        let info_local = PluginInfo::new(pid, "resource-test", [Permission::Control]);
        registry.register(info.clone()).await?;
        dispatcher.load_plugin(info).await?;

        // Test multiple rapid operation_s (should work within resource limit_s)
        for i in 0..10 {
            let test_data = header_byte_s(pid, format!("SBX:TEST {}", i).as_bytes());
            let result = dispatcher
                .dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, test_data)
                .await;
            // Don't assert succes_s/failure, just ensure no panic or crash
            println!("Operation {} result: {:?}", i, result.is_ok());
        }
    }
}

/// Test platform-specific integration behavior
#[test]
fn platform_specific_integration() {
    let os_statu_s = apply_policy(CorePolicy::Minimal);

    // Test platform-specific expectation_s
    #[cfg(all(windows, feature = "os_sandbox"))]
    {
        assert_eq!(os_statu_s, SandboxStatu_s::Applied);
        // windows should have Job Object restriction_s active
        println!("windows Job Object sandbox active");
    }

    #[cfg(all(target_os = "linux", feature = "os_sandbox"))]
    {
        assert_eq!(os_statu_s, SandboxStatu_s::Applied);
        // Linux should have resource limit_s and environment restriction_s
        if let Ok(policy) = env::var("SANDBOX_POLICY") {
            assert_eq!(policy, "minimal");
        }
        println!("Linux cooperative sandbox active");
    }

    #[cfg(all(target_os = "macos", feature = "os_sandbox"))]
    {
        assert_eq!(os_statu_s, SandboxStatu_s::Applied);
        // macOS should have resource limit_s and environment restriction_s
        if let Ok(policy) = env::var("SANDBOX_POLICY") {
            assert_eq!(policy, "minimal");
        }
        println!("macOS cooperative sandbox active");
    }

    #[cfg(not(feature = "os_sandbox"))]
    {
        assert_eq!(os_statu_s, SandboxStatu_s::Unsupported);
        println!("OS sandbox not available (feature disabled)");
    }

    // Stream sandbox should work regardles_s of OS sandbox statu_s
    let stream_policy = StreamPolicy::default();
    let guard = SandboxGuard::new(stream_policy);
    // SandboxGuard construction alway_s succeed_s
    println!("Stream sandbox guard created successfully");
}
