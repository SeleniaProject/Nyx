#![forbid(unsafe_code)]

/// Integration tests for nyx-stream plugin sandbox with cross-platform nyx-core sandbox

use nyx_stream::plugin_dispatch::PluginDispatcher;
use nyx_stream::plugin_registry::{PluginInfo, PluginRegistry, Permission};
use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_CONTROL};
use nyx_stream::plugin_sandbox::{SandboxPolicy as StreamPolicy, SandboxGuard};
use nyx_core::sandbox::{apply_policy, SandboxPolicy as CorePolicy, SandboxStatus};
use std::sync::Arc;
use std::env;

fn header_bytes(id: PluginId, data: &[u8]) -> Vec<u8> {
    let h = PluginHeader { id, flags: 0, data: data.to_vec() };
    let mut out = Vec::new();
    ciborium::ser::into_writer(&h, &mut out).expect("serialize header");
    out
}

/// Test integration between nyx-core and nyx-stream sandbox systems
#[tokio::test]
async fn cross_platform_sandbox_integration() {
    // Apply OS-level sandbox first
    let os_sandbox_status = apply_policy(CorePolicy::Minimal);
    
    // Create plugin framework sandbox
    let registry = Arc::new(PluginRegistry::new());
    let stream_policy = StreamPolicy::locked_down()
        .allow_connect_host("trusted.example.com")
        .allow_path_prefix(std::path::Path::new("/tmp/nyx"));
    
    let dispatcher = PluginDispatcher::new_with_sandbox(
        registry.clone(),
        StreamPolicy {
            allow_network: true,
            allow_fs: true,
            ..stream_policy
        }
    );

    // Register a test plugin
    let pid = PluginId(42);
    let info = PluginInfo::new(pid, "integration-test", [Permission::Control]);
    registry.register(info.clone()).await.unwrap();
    dispatcher.load_plugin(info).await.unwrap();

    // Test that both layers work together
    
    // 1. OS-level restrictions should be active (if supported)
    if os_sandbox_status == SandboxStatus::Applied {
        // Check environment variables set by OS sandbox
        if let Ok(policy) = env::var("SANDBOX_POLICY") {
            assert_eq!(policy, "minimal");
        }
        if let Ok(no_subprocess) = env::var("NO_SUBPROCESS") {
            assert_eq!(no_subprocess, "1");
        }
    }

    // 2. Stream-level restrictions should block unauthorized operations
    let blocked_connect = header_bytes(pid, b"SBX:CONNECT malicious.example.com:443");
    let result = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, blocked_connect).await;
    assert!(result.is_err(), "Stream sandbox should block unauthorized connections");

    let blocked_path = header_bytes(pid, b"SBX:OPEN /etc/shadow");
    let result = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, blocked_path).await;
    assert!(result.is_err(), "Stream sandbox should block unauthorized file access");

    // 3. Authorized operations should still work
    let allowed_connect = header_bytes(pid, b"SBX:CONNECT trusted.example.com:443");
    let result = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, allowed_connect).await;
    assert!(result.is_ok(), "Stream sandbox should allow authorized connections");

    let allowed_path = header_bytes(pid, b"SBX:OPEN /tmp/nyx/data.txt");
    let result = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, allowed_path).await;
    assert!(result.is_ok(), "Stream sandbox should allow authorized file access");
}

/// Test that strict OS sandbox affects plugin behavior
#[tokio::test]
async fn strict_os_sandbox_plugin_behavior() {
    // Apply strict OS-level sandbox
    let os_sandbox_status = apply_policy(CorePolicy::Strict);
    
    let registry = Arc::new(PluginRegistry::new());
    let stream_policy = StreamPolicy::default();
    let dispatcher = PluginDispatcher::new_with_sandbox(registry.clone(), stream_policy);

    let pid = PluginId(43);
    let info = PluginInfo::new(pid, "strict-test", [Permission::Control]);
    registry.register(info.clone()).await.unwrap();
    dispatcher.load_plugin(info).await.unwrap();

    // If OS sandbox is applied and strict, check environment
    if os_sandbox_status == SandboxStatus::Applied {
        if let Ok(policy) = env::var("SANDBOX_POLICY") {
            assert_eq!(policy, "strict");
        }
        if let Ok(no_network) = env::var("NO_NETWORK") {
            assert_eq!(no_network, "1");
        }
    }

    // Plugin operations should be more restricted under strict policy
    // This is a cooperative test - real plugins would check environment variables
    let test_data = header_bytes(pid, b"SBX:CONNECT example.com:80");
    let result = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, test_data).await;
    
    // Result depends on stream policy, but OS environment should influence behavior
    // This test verifies the integration without making plugins mandatory check env vars
    println!("Plugin operation result under strict OS sandbox: {:?}", result);
}

/// Test sandbox guard lifecycle with OS sandbox
#[test]
fn sandbox_guard_with_os_sandbox() {
    // Apply OS sandbox first
    let os_status = apply_policy(CorePolicy::Minimal);
    
    // Create stream sandbox guard with path allowlist
    // Use platform-appropriate paths and enable filesystem access
    #[cfg(windows)]
    let (allowed_prefix, allowed_path, denied_path) = (
        std::path::Path::new("C:\\temp"),
        "C:\\temp\\file.txt",
        "C:\\Windows\\System32\\config\\sam"
    );
    #[cfg(not(windows))]
    let (allowed_prefix, allowed_path, denied_path) = (
        std::path::Path::new("/tmp"),
        "/tmp/file.txt", 
        "/etc/passwd"
    );
    
    let stream_policy = StreamPolicy::permissive()  // Use permissive to enable FS
        .allow_connect_host("api.service.com")
        .allow_path_prefix(allowed_prefix);
    
    let guard = SandboxGuard::new(stream_policy);
    
    // Test path validation - should fail because denied_path is not under allowed prefix
    assert!(guard.check_open_path(denied_path).is_err());
    // Should succeed because allowed_path is under allowed prefix
    assert!(guard.check_open_path(allowed_path).is_ok());
    
    // Test host validation
    assert!(guard.check_connect("api.service.com:443").is_ok());
    assert!(guard.check_connect("malicious.com:80").is_err());
    
    // OS sandbox should be independent of stream guard lifecycle
    let os_status2 = apply_policy(CorePolicy::Minimal);
    assert_eq!(os_status, os_status2, "OS sandbox should be idempotent regardless of stream guard");
}

/// Test resource constraints affect plugin performance
#[tokio::test]
async fn resource_constraints_plugin_impact() {
    // Apply OS sandbox with resource limits
    let os_status = apply_policy(CorePolicy::Minimal);
    
    if os_status == SandboxStatus::Applied {
        // Create a plugin system
        let registry = Arc::new(PluginRegistry::new());
        let dispatcher = PluginDispatcher::new_with_sandbox(
            registry.clone(),
            StreamPolicy::default()
        );

        let pid = PluginId(44);
        let info = PluginInfo::new(pid, "resource-test", [Permission::Control]);
        registry.register(info.clone()).await.unwrap();
        dispatcher.load_plugin(info).await.unwrap();

        // Test multiple rapid operations (should work within resource limits)
        for i in 0..10 {
            let test_data = header_bytes(pid, format!("SBX:TEST {}", i).as_bytes());
            let result = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, test_data).await;
            // Don't assert success/failure, just ensure no panic or crash
            println!("Operation {} result: {:?}", i, result.is_ok());
        }
    }
}

/// Test platform-specific integration behavior
#[test]
fn platform_specific_integration() {
    let os_status = apply_policy(CorePolicy::Minimal);
    
    // Test platform-specific expectations
    #[cfg(all(windows, feature = "os_sandbox"))]
    {
        assert_eq!(os_status, SandboxStatus::Applied);
        // Windows should have Job Object restrictions active
        println!("Windows Job Object sandbox active");
    }
    
    #[cfg(all(target_os = "linux", feature = "os_sandbox"))]
    {
        assert_eq!(os_status, SandboxStatus::Applied);
        // Linux should have resource limits and environment restrictions
        if let Ok(policy) = env::var("SANDBOX_POLICY") {
            assert_eq!(policy, "minimal");
        }
        println!("Linux cooperative sandbox active");
    }
    
    #[cfg(all(target_os = "macos", feature = "os_sandbox"))]
    {
        assert_eq!(os_status, SandboxStatus::Applied);
        // macOS should have resource limits and environment restrictions
        if let Ok(policy) = env::var("SANDBOX_POLICY") {
            assert_eq!(policy, "minimal");
        }
        println!("macOS cooperative sandbox active");
    }
    
    #[cfg(not(feature = "os_sandbox"))]
    {
        assert_eq!(os_status, SandboxStatus::Unsupported);
        println!("OS sandbox not available (feature disabled)");
    }
    
    // Stream sandbox should work regardless of OS sandbox status
    let stream_policy = StreamPolicy::default();
    let guard = SandboxGuard::new(stream_policy);
    // SandboxGuard construction always succeeds
    println!("Stream sandbox guard created successfully");
}
