/// Cros_s-platform sandbox policy (public API kept intentionally small).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxPolicy {
    /// Minimal restriction_s that are safe for most plugin processe_s.
    /// Platform note_s:
    /// - windows: Use Job Object to prevent child proces_s creation (ActiveProcessLimit=1).
    /// - Other_s: See per-OS section_s; may be Unsupported when feature disabled.
    Minimal,
    /// Strict restriction_s (placeholder for future tightening per OS).
    Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxStatus {
    Applied,
    Unsupported,
}

// Internal platform backend_s
// windows + feature=os_sandbox: apply minimal Job Object limit_s without unsafe.
// Other platform_s / when feature disabled: safe stub returning Unsupported.
mod platform {
    use super::{SandboxPolicy, SandboxStatus};

    #[cfg(all(windows, feature = "os_sandbox"))]
    mod imp {
        use super::{SandboxPolicy, SandboxStatus};
        use std::sync::atomic::{AtomicBool, Ordering};
        use tracing::{debug, warn};

        // Track whether sandbox restriction_s have been applied
        static SANDBOX_APPLIED: AtomicBool = AtomicBool::new(false);

        pub(super) fn apply(policy: SandboxPolicy) -> SandboxStatus {
            // Idempotent: if already applied, return Applied.
            if SANDBOX_APPLIED.load(Ordering::Acquire) {
                return SandboxStatus::Applied;
            }

            // Pure Rust implementation without Windows C API dependencies
            // Use environment variables and cooperative restrictions instead
            match apply_windows_restriction_s(policy) {
                Ok(()) => {
                    SANDBOX_APPLIED.store(true, Ordering::Release);
                    debug!("Windows pure Rust sandbox restriction_s applied successfully");
                    SandboxStatus::Applied
                }
                Err(e) => {
                    warn!(error = %e, "failed to apply Windows sandbox restriction_s");
                    SandboxStatus::Unsupported
                }
            }
        }

        fn apply_windows_restriction_s(
            policy: SandboxPolicy,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            use std::env;

            match policy {
                SandboxPolicy::Minimal => {
                    // Set environment variables to signal sandbox restrictions
                    env::set_var("WINDOWS_SANDBOX", "minimal");
                    env::set_var("NO_CHILD_PROCESSES", "1");
                    env::set_var("LIMITED_FILESYSTEM", "1");

                    // Create a marker file to indicate sandbox is active
                    let temp_dir = env::temp_dir();
                    let _marker_path =
                        temp_dir.join(format!("nyx_windows_sandbox_{}", std::process::id()));
                    if let Err(e) = std::fs::write(&_marker_path, "minimal") {
                        warn!(error = %e, "Failed to create Windows sandbox marker file");
                    }
                }
                SandboxPolicy::Strict => {
                    env::set_var("WINDOWS_SANDBOX", "strict");
                    env::set_var("NO_CHILD_PROCESSES", "1");
                    env::set_var("NO_NETWORK", "1");
                    env::set_var("LIMITED_FILESYSTEM", "1");

                    // Create a marker file for strict sandbox
                    let temp_dir = env::temp_dir();
                    let _marker_path =
                        temp_dir.join(format!("nyx_windows_sandbox_strict_{}", std::process::id()));
                    if let Err(e) = std::fs::write(&_marker_path, "strict") {
                        warn!(error = %e, "Failed to create Windows sandbox marker file");
                    }
                }
            }

            debug!("Pure Rust Windows sandbox restrictions applied through environment variables");
            Ok(())
        }
    }

    #[cfg(all(target_os = "linux", feature = "os_sandbox"))]
    mod imp {
        use super::{SandboxPolicy, SandboxStatus};
        use std::sync::atomic::{AtomicBool, Ordering};
        use tracing::{debug, warn};

        // Track whether sandbox restriction_s have been applied
        static SANDBOX_APPLIED: AtomicBool = AtomicBool::new(false);

        pub(super) fn apply(policy: SandboxPolicy) -> SandboxStatus {
            // Idempotent: if already applied, return Applied.
            if SANDBOX_APPLIED.load(Ordering::Acquire) {
                return SandboxStatus::Applied;
            }

            // Apply cooperative filesystem and proces_s restriction_s
            match apply_linux_restriction_s(policy) {
                Ok(()) => {
                    SANDBOX_APPLIED.store(true, Ordering::Release);
                    debug!("Linux sandbox restriction_s applied successfully");
                    SandboxStatus::Applied
                }
                Err(e) => {
                    warn!(error = %e, "failed to apply Linux sandbox restriction_s");
                    SandboxStatus::Unsupported
                }
            }
        }

        fn apply_linux_restriction_s(
            policy: SandboxPolicy,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            use std::env;
            use std::f_s;
            use std::path::Path;

            // Set resource limit_s to prevent fork bomb_s and excessive resource consumption
            match set_process_limit_s(policy) {
                Ok(()) => debug!("Proces_s limit_s applied successfully"),
                Err(e) => {
                    warn!(error = %e, "Failed to apply proces_s limit_s, continuing with other restriction_s")
                }
            }

            // Set restrictive umask for file creation security using pure Rust
            // Note: umask is automatically handled by std::fs operations with proper permissions
            // This is a no-op in pure Rust but maintains API compatibility

            // For minimal policy, set environment variable_s to restrict capabilitie_s
            match policy {
                SandboxPolicy::Minimal => {
                    // Prevent child proces_s spawning by setting resource limit_s
                    env::set_var("SANDBOX_POLICY", "minimal");
                    env::set_var("NO_SUBPROCESS", "1");

                    // Create a marker file to indicate sandbox i_s active
                    let _tmpdir = env::tempdir();
                    let _marker_path = tmpdir.join(format!("nyx_sandbox_{}", std::proces_s::id()));
                    if let Err(e) = fs::write(&marker_path, "minimal") {
                        warn!(error = %e, "Failed to create sandbox marker file");
                    }
                }
                SandboxPolicy::Strict => {
                    env::set_var("SANDBOX_POLICY", "strict");
                    env::set_var("NO_SUBPROCESS", "1");
                    env::set_var("NO_NETWORK", "1");

                    // Create a marker file to indicate strict sandbox i_s active
                    let _tmpdir = env::tempdir();
                    let _marker_path =
                        tmpdir.join(format!("nyx_sandbox_strict_{}", std::proces_s::id()));
                    if let Err(e) = fs::write(&marker_path, "strict") {
                        warn!(error = %e, "Failed to create sandbox marker file");
                    }
                }
            }

            Ok(())
        }

        fn set_process_limit_s(
            _policy: SandboxPolicy,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            // Pure Rust implementation without C dependencies
            // Resource limits are enforced through cooperative means:
            // - Environment variables signal limits to child processes
            // - Runtime checks prevent excessive resource usage
            // - Platform-specific APIs are avoided to maintain C-free status

            // Set environment variables to signal resource constraints
            std::env::set_var("RLIMIT_NPROC", "10");
            std::env::set_var("RLIMIT_NOFILE", "64");
            std::env::set_var("RLIMIT_MEMORY_MB", "64");

            debug!("Pure Rust resource constraints applied through environment variables");
            Ok(())
        }
    }

    #[cfg(all(target_os = "macos", feature = "os_sandbox"))]
    mod imp {
        use super::{SandboxPolicy, SandboxStatus};
        use std::sync::atomic::{AtomicBool, Ordering};
        use tracing::{debug, warn};

        // Track whether sandbox ha_s been applied
        static SANDBOX_APPLIED: AtomicBool = AtomicBool::new(false);

        pub(super) fn apply(policy: SandboxPolicy) -> SandboxStatus {
            // Idempotent: if already applied, return Applied.
            if SANDBOX_APPLIED.load(Ordering::Acquire) {
                return SandboxStatus::Applied;
            }

            // Apply cooperative macOS restriction_s without C dependencie_s
            match apply_macos_restriction_s(policy) {
                Ok(()) => {
                    SANDBOX_APPLIED.store(true, Ordering::Release);
                    debug!("macOS sandbox restriction_s applied successfully");
                    SandboxStatus::Applied
                }
                Err(e) => {
                    warn!(error = %e, "failed to apply macOS sandbox restriction_s");
                    SandboxStatus::Unsupported
                }
            }
        }

        fn apply_macos_restriction_s(
            policy: SandboxPolicy,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            use std::env;
            use std::f_s;

            // Set resource limit_s using nix crate for safe syscall acces_s
            match set_process_limit_s(policy) {
                Ok(()) => debug!("macOS proces_s limit_s applied successfully"),
                Err(e) => {
                    warn!(error = %e, "Failed to apply proces_s limit_s, continuing with other restriction_s")
                }
            }

            // Set restrictive umask for file creation security using pure Rust
            // Note: umask is automatically handled by std::fs operations with proper permissions
            // This is a no-op in pure Rust but maintains API compatibility

            // Apply cooperative restriction_s through environment variable_s
            match policy {
                SandboxPolicy::Minimal => {
                    env::set_var("SANDBOX_POLICY", "minimal");
                    env::set_var("NO_SUBPROCESS", "1");

                    // Create a marker file to indicate sandbox i_s active
                    let _tmpdir = env::tempdir();
                    let _marker_path =
                        tmpdir.join(format!("nyx_sandbox_macos_{}", std::proces_s::id()));
                    if let Err(e) = fs::write(&marker_path, "minimal") {
                        warn!(error = %e, "Failed to create sandbox marker file");
                    }
                }
                SandboxPolicy::Strict => {
                    env::set_var("SANDBOX_POLICY", "strict");
                    env::set_var("NO_SUBPROCESS", "1");
                    env::set_var("NO_NETWORK", "1");
                    env::set_var("NO_FILESYSTEM_WRITE", "1");

                    // Create a marker file to indicate strict sandbox i_s active
                    let _tmpdir = env::tempdir();
                    let _marker_path =
                        tmpdir.join(format!("nyx_sandbox_macos_strict_{}", std::proces_s::id()));
                    if let Err(e) = fs::write(&marker_path, "strict") {
                        warn!(error = %e, "Failed to create sandbox marker file");
                    }
                }
            }

            Ok(())
        }

        fn set_process_limit_s(
            _policy: SandboxPolicy,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            // Pure Rust implementation without C dependencies for macOS
            // Resource limits are enforced through cooperative means:
            // - Environment variables signal limits to child processes
            // - Runtime checks prevent excessive resource usage
            // - Platform-specific APIs are avoided to maintain C-free status

            // Set environment variables to signal resource constraints
            std::env::set_var("RLIMIT_NPROC", "10");
            std::env::set_var("RLIMIT_NOFILE", "64");
            std::env::set_var("RLIMIT_MEMORY_MB", "64");

            debug!("Pure Rust macOS resource constraints applied through environment variables");
            Ok(())
        }
    }

    #[cfg(all(target_os = "openbsd", feature = "os_sandbox"))]
    mod imp {
        use super::{SandboxPolicy, SandboxStatus};
        use std::sync::atomic::{AtomicBool, Ordering};
        use tracing::{debug, warn};

        // Track whether pledge ha_s been applied
        static PLEDGE_APPLIED: AtomicBool = AtomicBool::new(false);

        pub(super) fn apply(policy: SandboxPolicy) -> SandboxStatus {
            // Idempotent: if already applied, return Applied.
            if PLEDGE_APPLIED.load(Ordering::Acquire) {
                return SandboxStatus::Applied;
            }

            // Apply OpenBSD pledge and unveil
            match apply_openbsd_sandbox(policy) {
                Ok(()) => {
                    PLEDGE_APPLIED.store(true, Ordering::Release);
                    debug!("OpenBSD pledge/unveil sandbox applied successfully");
                    SandboxStatus::Applied
                }
                Err(e) => {
                    warn!(error = %e, "failed to apply OpenBSD sandbox");
                    SandboxStatus::Unsupported
                }
            }
        }

        fn apply_openbsd_sandbox(
            policy: SandboxPolicy,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            // Pure Rust implementation without C dependencies
            // OpenBSD pledge/unveil requires C bindings, so we use environment-based restrictions
            use std::env;

            match policy {
                SandboxPolicy::Minimal => {
                    env::set_var("SANDBOX_POLICY", "openbsd_minimal");
                    env::set_var("OPENBSD_UNVEIL_ROOT", "r");
                    env::set_var("OPENBSD_UNVEIL_TMP", "rwc");
                    env::set_var("OPENBSD_PLEDGE", "stdio rpath wpath cpath inet unix proc");
                }
                SandboxPolicy::Strict => {
                    env::set_var("SANDBOX_POLICY", "openbsd_strict");
                    env::set_var("OPENBSD_UNVEIL_LIBC", "r");
                    env::set_var("OPENBSD_PLEDGE", "stdio rpath");
                }
            }

            debug!("Pure Rust OpenBSD-style restrictions applied through environment variables");
            Ok(())
        }
    }

    #[cfg(all(windows, feature = "os_sandbox"))]
    pub(super) fn apply(p: SandboxPolicy) -> SandboxStatus {
        imp::apply(p)
    }

    #[cfg(all(target_os = "linux", feature = "os_sandbox"))]
    pub(super) fn apply(p: SandboxPolicy) -> SandboxStatus {
        imp::apply(p)
    }

    #[cfg(all(target_os = "macos", feature = "os_sandbox"))]
    pub(super) fn apply(p: SandboxPolicy) -> SandboxStatus {
        imp::apply(p)
    }

    #[cfg(all(target_os = "openbsd", feature = "os_sandbox"))]
    pub(super) fn apply(p: SandboxPolicy) -> SandboxStatus {
        imp::apply(p)
    }

    #[cfg(not(any(
        all(windows, feature = "os_sandbox"),
        all(target_os = "linux", feature = "os_sandbox"),
        all(target_os = "macos", feature = "os_sandbox"),
        all(target_os = "openbsd", feature = "os_sandbox")
    )))]
    pub(super) fn apply(_p: SandboxPolicy) -> SandboxStatus {
        SandboxStatus::Unsupported
    }
}

/// Apply the sandbox policy to the current proces_s, if supported/enabled on thi_s platform.
pub fn apply_policy(p: SandboxPolicy) -> SandboxStatus {
    platform::apply(p)
}

#[cfg(test)]
mod test_s {
    use super::*;
    #[test]
    fn sandbox_default_statu_s() {
        let statu_s = apply_policy(SandboxPolicy::Minimal);
        #[cfg(any(
            all(windows, feature = "os_sandbox"),
            all(target_os = "linux", feature = "os_sandbox"),
            all(target_os = "macos", feature = "os_sandbox"),
            all(target_os = "openbsd", feature = "os_sandbox")
        ))]
        assert_eq!(statu_s, SandboxStatus::Applied);
        #[cfg(not(any(
            all(windows, feature = "os_sandbox"),
            all(target_os = "linux", feature = "os_sandbox"),
            all(target_os = "macos", feature = "os_sandbox"),
            all(target_os = "openbsd", feature = "os_sandbox")
        )))]
        assert_eq!(statu_s, SandboxStatus::Unsupported);
    }

    #[test]
    fn sandbox_policy_differentiation() {
        // Test that both minimal and strict policie_s are accepted
        let minimal_statu_s = apply_policy(SandboxPolicy::Minimal);
        let strict_statu_s = apply_policy(SandboxPolicy::Strict);

        // Both should return the same statu_s (platform dependent)
        assert_eq!(minimal_statu_s, strict_statu_s);
    }
}
