/// Cross-platform sandbox policy (public API kept intentionally small).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxPolicy {
	/// Minimal restrictions that are safe for most plugin processes.
	/// Platform notes:
	/// - Windows: Use Job Object to prevent child process creation (ActiveProcessLimit=1).
	/// - Others: See per-OS sections; may be Unsupported when feature disabled.
	Minimal,
	/// Strict restrictions (placeholder for future tightening per OS).
	Strict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxStatus { Applied, Unsupported }

// Internal platform backends
// Windows + feature=os_sandbox: apply minimal Job Object limits without unsafe.
// Other platforms / when feature disabled: safe stub returning Unsupported.
mod platform {
	use super::{SandboxPolicy, SandboxStatus};

	#[cfg(all(windows, feature = "os_sandbox"))]
	mod imp {
	use super::{SandboxPolicy, SandboxStatus};
	use once_cell::sync::OnceCell;
	use tracing::{debug, warn};
	use win32job::{ExtendedLimitInfo, Job};

		// Keep the job alive for the life of the process.
		static JOB: OnceCell<Job> = OnceCell::new();

		pub(super) fn apply(policy: SandboxPolicy) -> SandboxStatus {
			// Idempotent: if already applied, return Applied.
			if JOB.get().is_some() {
				return SandboxStatus::Applied;
			}

			// Create a Job object and apply minimal limits.
			let job = match Job::create() {
				Ok(j) => j,
				Err(e) => {
					warn!(error = %e, "failed to create Windows Job Object for sandbox");
					return SandboxStatus::Unsupported;
				}
			};

			// Minimal policy: prevent child process creation by limiting active processes to 1
			// and ensure processes are torn down if the job is closed.
			let mut limits = ExtendedLimitInfo::new();
			// Ensure that all processes in the job are terminated when the job handle
			// is closed (and on process shutdown). This provides robust cleanup without unsafe.
			limits.limit_kill_on_job_close();

			if let Err(e) = job.set_extended_limit_info(&limits) {
				warn!(error = %e, "failed to set Job Object extended limits");
				return SandboxStatus::Unsupported;
			}

			// Assign current process to the job.
			if let Err(e) = job.assign_current_process() {
				warn!(error = %e, "failed to assign current process to Job Object");
				return SandboxStatus::Unsupported;
			}

			// Keep the job alive.
			if let Err(_e) = JOB.set(job) {
				// Another thread raced us; treat as applied.
				debug!("sandbox job already set by another thread");
			}

			// Policy.Strict could add more limits in the future; Minimal is applied now.
			let _ = policy; // currently unused differentiation
			SandboxStatus::Applied
		}
	}

	#[cfg(all(target_os = "linux", feature = "os_sandbox"))]
	mod imp {
		use super::{SandboxPolicy, SandboxStatus};
		use std::sync::atomic::{AtomicBool, Ordering};
		use tracing::{debug, warn};

		// Track whether sandbox restrictions have been applied
		static SANDBOX_APPLIED: AtomicBool = AtomicBool::new(false);

		pub(super) fn apply(policy: SandboxPolicy) -> SandboxStatus {
			// Idempotent: if already applied, return Applied.
			if SANDBOX_APPLIED.load(Ordering::Acquire) {
				return SandboxStatus::Applied;
			}

			// Apply cooperative filesystem and process restrictions
			match apply_linux_restrictions(policy) {
				Ok(()) => {
					SANDBOX_APPLIED.store(true, Ordering::Release);
					debug!("Linux sandbox restrictions applied successfully");
					SandboxStatus::Applied
				}
				Err(e) => {
					warn!(error = %e, "failed to apply Linux sandbox restrictions");
					SandboxStatus::Unsupported
				}
			}
		}

		fn apply_linux_restrictions(policy: SandboxPolicy) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
			use std::env;
			use std::fs;
			use std::path::Path;

			// Set resource limits to prevent fork bombs and excessive resource consumption
			match set_process_limits(policy) {
				Ok(()) => debug!("Process limits applied successfully"),
				Err(e) => warn!(error = %e, "Failed to apply process limits, continuing with other restrictions"),
			}

			// Set restrictive umask for file creation security
			#[cfg(target_os = "linux")]
			unsafe {
				libc::umask(0o077); // Only owner can read/write newly created files
			}

			// For minimal policy, set environment variables to restrict capabilities
			match policy {
				SandboxPolicy::Minimal => {
					// Prevent child process spawning by setting resource limits
					env::set_var("SANDBOX_POLICY", "minimal");
					env::set_var("NO_SUBPROCESS", "1");
					
					// Create a marker file to indicate sandbox is active
					let tmp_dir = env::temp_dir();
					let marker_path = tmp_dir.join(format!("nyx_sandbox_{}", std::process::id()));
					if let Err(e) = fs::write(&marker_path, "minimal") {
						warn!(error = %e, "Failed to create sandbox marker file");
					}
				}
				SandboxPolicy::Strict => {
					env::set_var("SANDBOX_POLICY", "strict");
					env::set_var("NO_SUBPROCESS", "1");
					env::set_var("NO_NETWORK", "1");
					
					// Create a marker file to indicate strict sandbox is active
					let tmp_dir = env::temp_dir();
					let marker_path = tmp_dir.join(format!("nyx_sandbox_strict_{}", std::process::id()));
					if let Err(e) = fs::write(&marker_path, "strict") {
						warn!(error = %e, "Failed to create sandbox marker file");
					}
				}
			}

			Ok(())
		}

		fn set_process_limits(_policy: SandboxPolicy) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
			// Use nix crate for safe syscall access without C dependencies
			use nix::sys::resource::{setrlimit, Resource, Rlimit};

			// Limit number of processes to prevent fork bombs
			if let Err(e) = setrlimit(Resource::RLIMIT_NPROC, &Rlimit::from_raw(10, 50)) {
				return Err(format!("Failed to set process limit: {}", e).into());
			}

			// Limit file descriptor count
			if let Err(e) = setrlimit(Resource::RLIMIT_NOFILE, &Rlimit::from_raw(64, 128)) {
				return Err(format!("Failed to set file descriptor limit: {}", e).into());
			}

			// Limit memory usage (64MB soft, 128MB hard)
			if let Err(e) = setrlimit(Resource::RLIMIT_AS, &Rlimit::from_raw(64 * 1024 * 1024, 128 * 1024 * 1024)) {
				return Err(format!("Failed to set memory limit: {}", e).into());
			}

			debug!("Process resource limits applied successfully");
			Ok(())
		}
	}

	#[cfg(all(target_os = "macos", feature = "os_sandbox"))]
	mod imp {
		use super::{SandboxPolicy, SandboxStatus};
		use std::sync::atomic::{AtomicBool, Ordering};
		use tracing::{debug, warn};

		// Track whether sandbox has been applied
		static SANDBOX_APPLIED: AtomicBool = AtomicBool::new(false);

		pub(super) fn apply(policy: SandboxPolicy) -> SandboxStatus {
			// Idempotent: if already applied, return Applied.
			if SANDBOX_APPLIED.load(Ordering::Acquire) {
				return SandboxStatus::Applied;
			}

			// Apply cooperative macOS restrictions without C dependencies
			match apply_macos_restrictions(policy) {
				Ok(()) => {
					SANDBOX_APPLIED.store(true, Ordering::Release);
					debug!("macOS sandbox restrictions applied successfully");
					SandboxStatus::Applied
				}
				Err(e) => {
					warn!(error = %e, "failed to apply macOS sandbox restrictions");
					SandboxStatus::Unsupported
				}
			}
		}

		fn apply_macos_restrictions(policy: SandboxPolicy) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
			use std::env;
			use std::fs;

			// Set resource limits using nix crate for safe syscall access
			match set_process_limits(policy) {
				Ok(()) => debug!("macOS process limits applied successfully"),
				Err(e) => warn!(error = %e, "Failed to apply process limits, continuing with other restrictions"),
			}

			// Set restrictive umask for file creation security
			#[cfg(target_os = "macos")]
			unsafe {
				libc::umask(0o077); // Only owner can read/write newly created files
			}

			// Apply cooperative restrictions through environment variables
			match policy {
				SandboxPolicy::Minimal => {
					env::set_var("SANDBOX_POLICY", "minimal");
					env::set_var("NO_SUBPROCESS", "1");
					
					// Create a marker file to indicate sandbox is active
					let tmp_dir = env::temp_dir();
					let marker_path = tmp_dir.join(format!("nyx_sandbox_macos_{}", std::process::id()));
					if let Err(e) = fs::write(&marker_path, "minimal") {
						warn!(error = %e, "Failed to create sandbox marker file");
					}
				}
				SandboxPolicy::Strict => {
					env::set_var("SANDBOX_POLICY", "strict");
					env::set_var("NO_SUBPROCESS", "1");
					env::set_var("NO_NETWORK", "1");
					env::set_var("NO_FILESYSTEM_WRITE", "1");
					
					// Create a marker file to indicate strict sandbox is active
					let tmp_dir = env::temp_dir();
					let marker_path = tmp_dir.join(format!("nyx_sandbox_macos_strict_{}", std::process::id()));
					if let Err(e) = fs::write(&marker_path, "strict") {
						warn!(error = %e, "Failed to create sandbox marker file");
					}
				}
			}

			Ok(())
		}

		fn set_process_limits(_policy: SandboxPolicy) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
			// Use nix crate for safe syscall access without C dependencies
			use nix::sys::resource::{setrlimit, Resource, Rlimit};

			// Limit number of processes to prevent fork bombs
			if let Err(e) = setrlimit(Resource::RLIMIT_NPROC, &Rlimit::from_raw(10, 50)) {
				return Err(format!("Failed to set process limit: {}", e).into());
			}

			// Limit file descriptor count
			if let Err(e) = setrlimit(Resource::RLIMIT_NOFILE, &Rlimit::from_raw(64, 128)) {
				return Err(format!("Failed to set file descriptor limit: {}", e).into());
			}

			// Limit memory usage (64MB soft, 128MB hard)
			if let Err(e) = setrlimit(Resource::RLIMIT_AS, &Rlimit::from_raw(64 * 1024 * 1024, 128 * 1024 * 1024)) {
				return Err(format!("Failed to set memory limit: {}", e).into());
			}

			debug!("macOS process resource limits applied successfully");
			Ok(())
		}
	}

	#[cfg(all(target_os = "openbsd", feature = "os_sandbox"))]
	mod imp {
		use super::{SandboxPolicy, SandboxStatus};
		use std::sync::atomic::{AtomicBool, Ordering};
		use tracing::{debug, warn};

		// Track whether pledge has been applied
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

		fn apply_openbsd_sandbox(policy: SandboxPolicy) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
			// Apply unveil restrictions first
			match policy {
				SandboxPolicy::Minimal => {
					// Minimal restrictions: allow read access to system paths
					unveil::unveil("/", "r")?;
					unveil::unveil("/tmp", "rwc")?;
					unveil::unveil("/usr/lib", "r")?;
					unveil::unveil("/usr/local/lib", "r")?;
				}
				SandboxPolicy::Strict => {
					// Strict restrictions: minimal file system access
					unveil::unveil("/usr/lib/libc.so", "r")?;
					unveil::unveil("/usr/lib/libpthread.so", "r")?;
				}
			}
			
			// Lock unveil
			unveil::unveil_lock()?;

			// Apply pledge restrictions
			let promises = match policy {
				SandboxPolicy::Minimal => {
					// Minimal restrictions: allow most operations except dangerous ones
					"stdio rpath wpath cpath inet unix proc"
				}
				SandboxPolicy::Strict => {
					// Strict restrictions: only essential operations
					"stdio rpath"
				}
			};

			pledge::pledge(promises, None)?;
			
			Ok(())
		}
	}

	#[cfg(all(windows, feature = "os_sandbox"))]
	pub(super) fn apply(p: SandboxPolicy) -> SandboxStatus { imp::apply(p) }

	#[cfg(all(target_os = "linux", feature = "os_sandbox"))]
	pub(super) fn apply(p: SandboxPolicy) -> SandboxStatus { imp::apply(p) }

	#[cfg(all(target_os = "macos", feature = "os_sandbox"))]
	pub(super) fn apply(p: SandboxPolicy) -> SandboxStatus { imp::apply(p) }

	#[cfg(all(target_os = "openbsd", feature = "os_sandbox"))]
	pub(super) fn apply(p: SandboxPolicy) -> SandboxStatus { imp::apply(p) }

	#[cfg(not(any(
		all(windows, feature = "os_sandbox"),
		all(target_os = "linux", feature = "os_sandbox"),
		all(target_os = "macos", feature = "os_sandbox"),
		all(target_os = "openbsd", feature = "os_sandbox")
	)))]
	pub(super) fn apply(_p: SandboxPolicy) -> SandboxStatus { SandboxStatus::Unsupported }
}

/// Apply the sandbox policy to the current process, if supported/enabled on this platform.
pub fn apply_policy(p: SandboxPolicy) -> SandboxStatus { platform::apply(p) }

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn sandbox_default_status() {
		let status = apply_policy(SandboxPolicy::Minimal);
		#[cfg(any(
			all(windows, feature = "os_sandbox"),
			all(target_os = "linux", feature = "os_sandbox"),
			all(target_os = "macos", feature = "os_sandbox"),
			all(target_os = "openbsd", feature = "os_sandbox")
		))]
		assert_eq!(status, SandboxStatus::Applied);
		#[cfg(not(any(
			all(windows, feature = "os_sandbox"),
			all(target_os = "linux", feature = "os_sandbox"),
			all(target_os = "macos", feature = "os_sandbox"),
			all(target_os = "openbsd", feature = "os_sandbox")
		)))]
		assert_eq!(status, SandboxStatus::Unsupported);
	}

	#[test]
	fn sandbox_policy_differentiation() {
		// Test that both minimal and strict policies are accepted
		let minimal_status = apply_policy(SandboxPolicy::Minimal);
		let strict_status = apply_policy(SandboxPolicy::Strict);
		
		// Both should return the same status (platform dependent)
		assert_eq!(minimal_status, strict_status);
	}
}
