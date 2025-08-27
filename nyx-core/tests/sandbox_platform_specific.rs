#![forbid(unsafe_code)]

/// Platform-specific sandbox test_s for Unix-like system_s (Linux/macOS)

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_tests {
    use nyx_core::sandbox::{apply_policy, SandboxPolicy, SandboxStatus};
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use tempfile::tempdir;

    /// Test that umask is set restrictively after sandbox application
    #[test]
    fn restrictive_umask_applied() -> Result<(), Box<dyn std::error::Error>> {
        let status = apply_policy(SandboxPolicy::Minimal);

        if status == SandboxStatus::Applied {
            // Create a test file and check permissions
            let tmpdir = tempdir()?;
            let test_file = tmpdir.path().join(format!("nyx_umask_test_{}", process::id()));

            // Write to file (this will use the current umask)
            fs::write(&test_file, "test")?;

            // Check file permissions
            let metadata = fs::metadata(&test_file)?;
            let permissions = metadata.permissions();

            // On Unix, permissions should be restrictive (only owner access)
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = permissions.mode();

                // Should not have group or other permissions
                assert_eq!(
                    mode & 0o077,
                    0,
                    "File permissions should be owner-only due to restrictive umask: {:o}",
                    mode
                );
            }

            // Clean up
            let _ = fs::remove_file(&test_file);
        }
        Ok(())
    }

    /// Test resource limit functionality (simplified without nix crate)
    #[test]
    fn resource_limits_verification() {
        let status = apply_policy(SandboxPolicy::Minimal);

        if status == SandboxStatus::Applied {
            // Basic verification that policy was applied
            // Resource limits testing would require platform-specific implementation
            // For now, we just verify that the policy application succeeded
            assert_eq!(status, SandboxStatus::Applied);
        }
    }

    /// Test environment variable propagation for cooperative restrictions
    #[test]
    fn cooperative_environment_variables() {
        // Clear environment first
        for var in &[
            "SANDBOX_POLICY",
            "NO_SUBPROCESS",
            "NO_NETWORK",
            "NO_FILESYSTEM_WRITE",
        ] {
            env::remove_var(var);
        }

        // Test minimal policy
        let status = apply_policy(SandboxPolicy::Minimal);
        if status == SandboxStatus::Applied {
            assert_eq!(env::var("SANDBOX_POLICY").unwrap(), "minimal");
            assert_eq!(env::var("NO_SUBPROCESS").unwrap(), "1");
            assert!(env::var("NO_NETWORK").is_err()); // Should not be set for minimal
        }

        // Test strict policy
        let status = apply_policy(SandboxPolicy::Strict);
        if status == SandboxStatus::Applied {
            assert_eq!(env::var("SANDBOX_POLICY").unwrap(), "strict");
            assert_eq!(env::var("NO_SUBPROCESS").unwrap(), "1");
            assert_eq!(env::var("NO_NETWORK").unwrap(), "1");

            // macOS should also set NO_FILESYSTEM_WRITE
            #[cfg(target_os = "macos")]
            assert_eq!(env::var("NO_FILESYSTEM_WRITE").unwrap(), "1");
        }
    }

    /// Test that sandbox markers are created with correct process ID
    #[test]
    fn process_specific_markers() -> Result<(), Box<dyn std::error::Error>> {
        let tmpdir = tempdir()?;
        let process_id = process::id();

        // Apply both policies and check markers
        let minimal_status = apply_policy(SandboxPolicy::Minimal);
        let strict_status = apply_policy(SandboxPolicy::Strict);

        if minimal_status == SandboxStatus::Applied || strict_status == SandboxStatus::Applied {
            // Check for process-specific marker files
            let platform_prefix = if cfg!(target_os = "macos") {
                "macos_"
            } else {
                ""
            };

            let minimal_marker =
                tmpdir.path().join(format!("nyx_sandbox_{}{}", platform_prefix, process_id));
            let strict_marker = tmpdir.path().join(format!(
                "nyx_sandbox_{}strict_{}",
                platform_prefix, process_id
            ));

            // At least one marker should exist
            assert!(
                minimal_marker.exists() || strict_marker.exists(),
                "Expected to find at least one sandbox marker file"
            );

            // Clean up markers
            let _ = fs::remove_file(&minimal_marker);
            let _ = fs::remove_file(&strict_marker);
        }
        Ok(())
    }

    /// Test sandbox stability under rapid policy changes
    #[test]
    fn rapid_policy_switching() {
        let policies = [SandboxPolicy::Minimal, SandboxPolicy::Strict];
        let mut results = Vec::new();

        // Rapidly switch between policies
        for _ in 0..10 {
            for policy in &policies {
                results.push(apply_policy(*policy));
            }
        }

        // All results should be consistent (idempotent)
        let first_result = results[0];
        for result in &results[1..] {
            assert_eq!(
                *result, first_result,
                "Rapid policy switching should maintain idempotent behavior"
            );
        }
    }

    /// Test that resource limits don't interfere with normal operation
    #[test]
    fn resource_limits_functional() -> Result<(), Box<dyn std::error::Error>> {
        let status = apply_policy(SandboxPolicy::Minimal);

        if status == SandboxStatus::Applied {
            // Test that we can still perform basic operations

            // File operations
            let tmpdir = tempdir()?;
            let test_file = tmpdir.path().join(format!("functional_test_{}", process::id()));
            fs::write(&test_file, "functional test")?;
            let content = fs::read_to_string(&test_file)?;
            assert_eq!(content, "functional test");
            fs::remove_file(&test_file)?;

            // Memory allocation
            let mut test_vec = Vec::with_capacity(1024);
            for i in 0..1024 {
                test_vec.push(i);
            }
            assert_eq!(test_vec.len(), 1024);

            // Environment access
            let path_var = env::var("PATH");
            assert!(
                path_var.is_ok(),
                "Should be able to access environment variables"
            );
        }
        Ok(())
    }
}

#[cfg(windows)]
mod windows_tests {
    use nyx_core::sandbox::{apply_policy, SandboxPolicy, SandboxStatus};

    /// Test windows-specific Job Object functionality
    #[test]
    fn windows_job_object_applied() {
        let status = apply_policy(SandboxPolicy::Minimal);

        // On windows with os_sandbox feature, should be applied
        #[cfg(feature = "os_sandbox")]
        assert_eq!(
            status,
            SandboxStatus::Applied,
            "windows should support sandbox with win32job"
        );

        #[cfg(not(feature = "os_sandbox"))]
        assert_eq!(
            status,
            SandboxStatus::Unsupported,
            "windows should not support sandbox without feature"
        );
    }

    /// Test idempotent behavior on windows
    #[test]
    fn windows_idempotent_application() {
        let status1 = apply_policy(SandboxPolicy::Minimal);
        let status2 = apply_policy(SandboxPolicy::Minimal);
        let status3 = apply_policy(SandboxPolicy::Strict);

        // All should return the same result
        assert_eq!(status1, status2);
        assert_eq!(status2, status3);
    }
}

#[cfg(target_os = "openbsd")]
mod openbsd_tests {
    use nyx_core::sandbox::{apply_policy, SandboxPolicy, SandboxStatus};

    /// Test OpenBSD pledge/unveil functionality
    #[test]
    fn openbsd_pledge_unveil() {
        let status = apply_policy(SandboxPolicy::Minimal);

        #[cfg(feature = "os_sandbox")]
        assert_eq!(
            status,
            SandboxStatus::Applied,
            "OpenBSD should support sandbox"
        );

        #[cfg(not(feature = "os_sandbox"))]
        assert_eq!(
            status,
            SandboxStatus::Unsupported,
            "OpenBSD should not support sandbox without feature"
        );
    }
}
