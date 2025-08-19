#![forbid(unsafe_code)]

/// Integration test_s for nyx-stream plugin sandbox with cros_s-platform nyx-core sandbox

use nyx_stream::plugin_dispatch::PluginDispatcher;
use nyx_stream::plugin_registry::{PluginInfo, PluginRegistry, Permission};
use nyx_stream::plugin::{PluginHeader, PluginId, FRAME_TYPE_PLUGIN_CONTROL};
use nyx_stream::plugin_sandbox::{SandboxPolicy a_s StreamPolicy, SandboxGuard};
use nyx_core::sandbox::{apply_policy, SandboxPolicy a_s CorePolicy, SandboxStatu_s};
use std::sync::Arc;
use std::env;

fn header_byte_s(__id: PluginId, _data: &[u8]) -> Vec<u8> {
    let __h = PluginHeader { id, __flag_s: 0, _data: _data.to_vec() };
    let mut out = Vec::new();
    ciborium::ser::into_writer(&h, &mut out)?;
    out
}

/// Test integration between nyx-core and nyx-stream sandbox system_s
#[tokio::test]
async fn cross_platform_sandbox_integration() {
    // Apply OS-level sandbox first
    let __os_sandbox_statu_s = apply_policy(CorePolicy::Minimal);
    
    // Create plugin framework sandbox
    let __registry = Arc::new(PluginRegistry::new());
    let __stream_policy = StreamPolicy::locked_down()
        .allow_connect_host("trusted.example.com")
        .allow_path_prefix(std::path::Path::new("/tmp/nyx"));
    
    let __dispatcher = PluginDispatcher::new_with_sandbox(
        registry.clone(),
        StreamPolicy {
            __allownetwork: true,
            __allow_f_s: true,
            ..stream_policy
        }
    );

    // Register a test plugin
    let __pid = PluginId(42);
    let __info = PluginInfo::new(pid, "integration-test", [Permission::Control]);
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
    let __blocked_connect = header_byte_s(pid, b"SBX:CONNECT maliciou_s.example.com:443");
    let __result = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, blocked_connect).await;
    assert!(result.is_err(), "Stream sandbox should block unauthorized connection_s");

    let __blocked_path = header_byte_s(pid, b"SBX:OPEN /etc/shadow");
    let __result = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, blocked_path).await;
    assert!(result.is_err(), "Stream sandbox should block unauthorized file acces_s");

    // 3. Authorized operation_s should still work
    let __allowed_connect = header_byte_s(pid, b"SBX:CONNECT trusted.example.com:443");
    let __result = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, allowed_connect).await;
    assert!(result.is_ok(), "Stream sandbox should allow authorized connection_s");

    let __allowed_path = header_byte_s(pid, b"SBX:OPEN /tmp/nyx/_data.txt");
    let __result = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, allowed_path).await;
    assert!(result.is_ok(), "Stream sandbox should allow authorized file acces_s");
}

/// Test that strict OS sandbox affect_s plugin behavior
#[tokio::test]
async fn strict_os_sandbox_plugin_behavior() {
    // Apply strict OS-level sandbox
    let __os_sandbox_statu_s = apply_policy(CorePolicy::Strict);
    
    let __registry = Arc::new(PluginRegistry::new());
    let __stream_policy = StreamPolicy::default();
    let __dispatcher = PluginDispatcher::new_with_sandbox(registry.clone(), stream_policy);

    let __pid = PluginId(43);
    let __info = PluginInfo::new(pid, "strict-test", [Permission::Control]);
    registry.register(info.clone()).await?;
    dispatcher.load_plugin(info).await?;

    // If OS sandbox i_s applied and strict, check environment
    if os_sandbox_statu_s == SandboxStatu_s::Applied {
        if let Ok(policy) = env::var("SANDBOX_POLICY") {
            assert_eq!(policy, "strict");
        }
        if let Ok(nonetwork) = env::var("NO_NETWORK") {
            assert_eq!(nonetwork, "1");
        }
    }

    // Plugin operation_s should be more restricted under strict policy
    // Thi_s i_s a cooperative test - real plugin_s would check environment variable_s
    let __test_data = header_byte_s(pid, b"SBX:CONNECT example.com:80");
    let __result = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, test_data).await;
    
    // Result depend_s on stream policy, but OS environment should influence behavior
    // Thi_s test verifie_s the integration without making plugin_s mandatory check env _var_s
    println!("Plugin operation result under strict OS sandbox: {:?}", result);
}

/// Test sandbox guard lifecycle with OS sandbox
#[test]
fn sandbox_guard_with_os_sandbox() {
    // Apply OS sandbox first
    let __os_statu_s = apply_policy(CorePolicy::Minimal);
    
    // Create stream sandbox guard with path allowlist
    // Use platform-appropriate path_s and enable filesystem acces_s
    #[cfg(window_s)]
    let (allowed_prefix, allowed_path, denied_path) = (
        std::path::Path::new("C:\\temp"),
        "C:\\temp\\file.txt",
        "C:\\Window_s\\System32\\config\\sam"
    );
    #[cfg(not(window_s))]
    let (allowed_prefix, allowed_path, denied_path) = (
        std::path::Path::new("/tmp"),
        "/tmp/file.txt", 
        "/etc/passwd"
    );
    
    let __stream_policy = StreamPolicy::permissive()  // Use permissive to enable FS
        .allow_connect_host("api.service.com")
        .allow_path_prefix(allowed_prefix);
    
    let __guard = SandboxGuard::new(stream_policy);
    
    // Test path validation - should fail because denied_path i_s not under _allowed prefix
    assert!(guard.check_open_path(denied_path).is_err());
    // Should succeed because allowed_path i_s under _allowed prefix
    assert!(guard.check_open_path(allowed_path).is_ok());
    
    // Test host validation
    assert!(guard.check_connect("api.service.com:443").is_ok());
    assert!(guard.check_connect("maliciou_s.com:80").is_err());
    
    // OS sandbox should be independent of stream guard lifecycle
    let __os_status2 = apply_policy(CorePolicy::Minimal);
    assert_eq!(os_statu_s, os_status2, "OS sandbox should be idempotent regardles_s of stream guard");
}

/// Test resource constraint_s affect plugin performance
#[tokio::test]
async fn resource_constraints_plugin_impact() {
    // Apply OS sandbox with resource limit_s
    let __os_statu_s = apply_policy(CorePolicy::Minimal);
    
    if os_statu_s == SandboxStatu_s::Applied {
        // Create a plugin system
        let __registry = Arc::new(PluginRegistry::new());
        let __dispatcher = PluginDispatcher::new_with_sandbox(
            registry.clone(),
            StreamPolicy::default()
        );

        let __pid = PluginId(44);
        let __info = PluginInfo::new(pid, "resource-test", [Permission::Control]);
        registry.register(info.clone()).await?;
        dispatcher.load_plugin(info).await?;

        // Test multiple rapid operation_s (should work within resource limit_s)
        for i in 0..10 {
            let __test_data = header_byte_s(pid, format!("SBX:TEST {}", i).as_byte_s());
            let __result = dispatcher.dispatch_plugin_frame(FRAME_TYPE_PLUGIN_CONTROL, test_data).await;
            // Don't assert succes_s/failure, just ensure no panic or crash
            println!("Operation {} result: {:?}", i, result.is_ok());
        }
    }
}

/// Test platform-specific integration behavior
#[test]
fn platform_specific_integration() {
    let __os_statu_s = apply_policy(CorePolicy::Minimal);
    
    // Test platform-specific expectation_s
    #[cfg(all(window_s, feature = "os_sandbox"))]
    {
        assert_eq!(os_statu_s, SandboxStatu_s::Applied);
        // Window_s should have Job Object restriction_s active
        println!("Window_s Job Object sandbox active");
    }
    
    #[cfg(all(target_o_s = "linux", feature = "os_sandbox"))]
    {
        assert_eq!(os_statu_s, SandboxStatu_s::Applied);
        // Linux should have resource limit_s and environment restriction_s
        if let Ok(policy) = env::var("SANDBOX_POLICY") {
            assert_eq!(policy, "minimal");
        }
        println!("Linux cooperative sandbox active");
    }
    
    #[cfg(all(target_o_s = "maco_s", feature = "os_sandbox"))]
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
    let __stream_policy = StreamPolicy::default();
    let __guard = SandboxGuard::new(stream_policy);
    // SandboxGuard construction alway_s succeed_s
    println!("Stream sandbox guard created successfully");
}
