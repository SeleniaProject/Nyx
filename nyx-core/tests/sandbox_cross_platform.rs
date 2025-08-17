#![forbid(unsafe_code)]

use nyx_core::sandbox::{apply_policy, SandboxPolicy, SandboxStatus};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

/// Test that sandbox application is idempotent
#[test]
fn sandbox_idempotent_application() {
    let status1 = apply_policy(SandboxPolicy::Minimal);
    let status2 = apply_policy(SandboxPolicy::Minimal);
    
    // Both calls should succeed (or both fail consistently)
    assert_eq!(status1, status2);
}

/// Test that environment variables are set correctly for minimal policy
#[test]
fn minimal_policy_environment_setup() {
    // Clear any existing sandbox environment
    env::remove_var("SANDBOX_POLICY");
    env::remove_var("NO_SUBPROCESS");
    env::remove_var("NO_NETWORK");
    
    let status = apply_policy(SandboxPolicy::Minimal);
    
    // On platforms where sandbox is supported, check environment
    if status == SandboxStatus::Applied {
        assert_eq!(env::var("SANDBOX_POLICY").unwrap(), "minimal");
        assert_eq!(env::var("NO_SUBPROCESS").unwrap(), "1");
        
        // NO_NETWORK should not be set for minimal policy
        assert!(env::var("NO_NETWORK").is_err());
    }
}

/// Test that environment variables are set correctly for strict policy
#[test]
fn strict_policy_environment_setup() {
    // Clear any existing sandbox environment
    env::remove_var("SANDBOX_POLICY");
    env::remove_var("NO_SUBPROCESS");
    env::remove_var("NO_NETWORK");
    
    let status = apply_policy(SandboxPolicy::Strict);
    
    // On platforms where sandbox is supported, check environment
    if status == SandboxStatus::Applied {
        assert_eq!(env::var("SANDBOX_POLICY").unwrap(), "strict");
        assert_eq!(env::var("NO_SUBPROCESS").unwrap(), "1");
        assert_eq!(env::var("NO_NETWORK").unwrap(), "1");
    }
}

/// Test that sandbox marker files are created appropriately
#[test]
fn sandbox_marker_file_creation() {
    let tmp_dir = env::temp_dir();
    let process_id = process::id();
    
    // Apply sandbox policy
    let status = apply_policy(SandboxPolicy::Minimal);
    
    if status == SandboxStatus::Applied {
        // Check for platform-specific marker files
        let possible_markers = vec![
            tmp_dir.join(format!("nyx_sandbox_{}", process_id)),
            tmp_dir.join(format!("nyx_sandbox_macos_{}", process_id)),
        ];
        
        let mut found_marker = false;
        for marker_path in possible_markers {
            if marker_path.exists() {
                let contents = fs::read_to_string(&marker_path).unwrap();
                assert_eq!(contents, "minimal");
                found_marker = true;
                
                // Clean up
                let _ = fs::remove_file(&marker_path);
                break;
            }
        }
        
        // On supported platforms, we should find a marker
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        assert!(found_marker, "Expected to find sandbox marker file");
    }
}

/// Test that strict policy creates appropriate marker
#[test]
fn strict_policy_marker_file() {
    let tmp_dir = env::temp_dir();
    let process_id = process::id();
    
    // Apply strict sandbox policy
    let status = apply_policy(SandboxPolicy::Strict);
    
    if status == SandboxStatus::Applied {
        // Check for platform-specific strict marker files
        let possible_markers = vec![
            tmp_dir.join(format!("nyx_sandbox_strict_{}", process_id)),
            tmp_dir.join(format!("nyx_sandbox_macos_strict_{}", process_id)),
        ];
        
        let mut found_marker = false;
        for marker_path in possible_markers {
            if marker_path.exists() {
                let contents = fs::read_to_string(&marker_path).unwrap();
                assert_eq!(contents, "strict");
                found_marker = true;
                
                // Clean up
                let _ = fs::remove_file(&marker_path);
                break;
            }
        }
        
        // On supported platforms, we should find a marker
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        assert!(found_marker, "Expected to find strict sandbox marker file");
    }
}

/// Test sandbox behavior consistency across policy types
#[test]
fn policy_type_consistency() {
    let minimal_status = apply_policy(SandboxPolicy::Minimal);
    let strict_status = apply_policy(SandboxPolicy::Strict);
    
    // Both policies should behave consistently (both applied or both unsupported)
    // on the same platform
    assert_eq!(
        minimal_status == SandboxStatus::Applied,
        strict_status == SandboxStatus::Applied,
        "Policy application should be consistent across policy types"
    );
}

/// Test that multiple sandbox applications don't interfere
#[test]
fn multiple_applications_safe() {
    // Apply different policies multiple times
    let results: Vec<SandboxStatus> = vec![
        apply_policy(SandboxPolicy::Minimal),
        apply_policy(SandboxPolicy::Strict), 
        apply_policy(SandboxPolicy::Minimal),
        apply_policy(SandboxPolicy::Strict),
    ];
    
    // All should return the same status (idempotent behavior)
    let first_result = results[0];
    for result in &results[1..] {
        assert_eq!(*result, first_result, "Multiple sandbox applications should be idempotent");
    }
}

/// Integration test for resource limits (if applicable)
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn resource_limits_applied() {
    use nix::sys::resource::{getrlimit, Resource};
    
    let status = apply_policy(SandboxPolicy::Minimal);
    
    if status == SandboxStatus::Applied {
        // Check that resource limits were applied
        if let Ok(process_limit) = getrlimit(Resource::RLIMIT_NPROC) {
            // Should have some reasonable limit set
            assert!(process_limit.soft().unwrap_or(u64::MAX) <= 50, 
                    "Process limit should be restricted");
        }
        
        if let Ok(fd_limit) = getrlimit(Resource::RLIMIT_NOFILE) {
            // Should have file descriptor limit
            assert!(fd_limit.soft().unwrap_or(u64::MAX) <= 128,
                    "File descriptor limit should be restricted");
        }
    }
}

/// Test platform detection and feature availability
#[test]
fn platform_feature_detection() {
    let status = apply_policy(SandboxPolicy::Minimal);
    
    // Check that status aligns with platform expectations
    #[cfg(all(windows, feature = "os_sandbox"))]
    assert_eq!(status, SandboxStatus::Applied, "Windows with os_sandbox feature should support sandbox");
    
    #[cfg(all(target_os = "linux", feature = "os_sandbox"))]
    assert_eq!(status, SandboxStatus::Applied, "Linux with os_sandbox feature should support sandbox");
    
    #[cfg(all(target_os = "macos", feature = "os_sandbox"))]
    assert_eq!(status, SandboxStatus::Applied, "macOS with os_sandbox feature should support sandbox");
    
    #[cfg(not(feature = "os_sandbox"))]
    assert_eq!(status, SandboxStatus::Unsupported, "Without os_sandbox feature, sandbox should be unsupported");
}

/// Performance test - sandbox application should be fast
#[test]
fn sandbox_application_performance() {
    use std::time::Instant;
    
    let start = Instant::now();
    let _status = apply_policy(SandboxPolicy::Minimal);
    let duration = start.elapsed();
    
    // Sandbox application should complete quickly (under 100ms)
    assert!(duration.as_millis() < 100, 
            "Sandbox application took too long: {:?}", duration);
}
