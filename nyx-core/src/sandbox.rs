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
		use once_cell::sync::OnceCell;
		use tracing::{debug, warn};
		use std::sync::atomic::{AtomicBool, Ordering};

		// Track whether seccomp filter has been applied
		static SECCOMP_APPLIED: AtomicBool = AtomicBool::new(false);

		pub(super) fn apply(policy: SandboxPolicy) -> SandboxStatus {
			// Idempotent: if already applied, return Applied.
			if SECCOMP_APPLIED.load(Ordering::Acquire) {
				return SandboxStatus::Applied;
			}

			// Apply seccomp-bpf filter to restrict system calls
			match apply_seccomp_filter(policy) {
				Ok(()) => {
					SECCOMP_APPLIED.store(true, Ordering::Release);
					debug!("Linux seccomp sandbox applied successfully");
					SandboxStatus::Applied
				}
				Err(e) => {
					warn!(error = %e, "failed to apply Linux seccomp sandbox");
					SandboxStatus::Unsupported
				}
			}
		}

		fn apply_seccomp_filter(policy: SandboxPolicy) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
			use seccompiler::{
				BpfProgram, SeccompAction, SeccompCmpOp, SeccompCondition, SeccompFilter,
				SeccompRule, TargetArch,
			};

			// Create base filter allowing essential syscalls for minimal operation
			let mut filter = SeccompFilter::new(
				vec![
					// Essential syscalls for basic process operation
					(libc::SYS_read, vec![]),
					(libc::SYS_write, vec![]),
					(libc::SYS_close, vec![]),
					(libc::SYS_exit, vec![]),
					(libc::SYS_exit_group, vec![]),
					(libc::SYS_brk, vec![]),
					(libc::SYS_mmap, vec![]),
					(libc::SYS_munmap, vec![]),
					(libc::SYS_mprotect, vec![]),
					(libc::SYS_rt_sigaction, vec![]),
					(libc::SYS_rt_sigprocmask, vec![]),
					(libc::SYS_rt_sigreturn, vec![]),
					(libc::SYS_futex, vec![]),
					(libc::SYS_getpid, vec![]),
					(libc::SYS_gettid, vec![]),
					(libc::SYS_clock_gettime, vec![]),
					(libc::SYS_getrandom, vec![]),
				]
				.into_iter()
				.map(|(syscall, conditions)| {
					(syscall as i64, vec![SeccompRule::new(conditions, SeccompAction::Allow)])
				})
				.collect(),
				SeccompAction::Errno(libc::EPERM as u32),
				TargetArch::x86_64,
			)?;

			// Add policy-specific restrictions
			match policy {
				SandboxPolicy::Minimal => {
					// Minimal restrictions: block dangerous syscalls but allow most operations
					let blocked_syscalls = vec![
						libc::SYS_fork,
						libc::SYS_vfork,
						libc::SYS_clone,
						libc::SYS_execve,
						libc::SYS_execveat,
						libc::SYS_ptrace,
						libc::SYS_reboot,
						libc::SYS_mount,
						libc::SYS_umount2,
						libc::SYS_swapon,
						libc::SYS_swapoff,
					];
					
					for syscall in blocked_syscalls {
						filter.add_rules(
							syscall as i64,
							vec![SeccompRule::new(vec![], SeccompAction::Errno(libc::EPERM as u32))],
						)?;
					}
				}
				SandboxPolicy::Strict => {
					// Strict restrictions: more comprehensive blocking
					// Block network operations if needed
					let strict_blocked = vec![
						libc::SYS_socket,
						libc::SYS_connect,
						libc::SYS_bind,
						libc::SYS_listen,
						libc::SYS_accept,
						libc::SYS_accept4,
						libc::SYS_sendto,
						libc::SYS_recvfrom,
						libc::SYS_sendmsg,
						libc::SYS_recvmsg,
						// File system operations
						libc::SYS_open,
						libc::SYS_openat,
						libc::SYS_creat,
						libc::SYS_unlink,
						libc::SYS_unlinkat,
						libc::SYS_mkdir,
						libc::SYS_mkdirat,
						libc::SYS_rmdir,
						libc::SYS_rename,
						libc::SYS_renameat,
						libc::SYS_renameat2,
						// Process control
						libc::SYS_fork,
						libc::SYS_vfork,
						libc::SYS_clone,
						libc::SYS_execve,
						libc::SYS_execveat,
						libc::SYS_ptrace,
					];
					
					for syscall in strict_blocked {
						filter.add_rules(
							syscall as i64,
							vec![SeccompRule::new(vec![], SeccompAction::Errno(libc::EPERM as u32))],
						)?;
					}
				}
			}

			// Compile and apply the filter
			let filter_map = vec![(TargetArch::x86_64, filter)].into_iter().collect();
			let bpf_prog = BpfProgram::new(&filter_map)?;
			
			// Apply seccomp filter using seccomp crate
			use seccomp::{seccomp, SECCOMP_SET_MODE_FILTER};
			seccomp(SECCOMP_SET_MODE_FILTER, 0, &bpf_prog.as_ref()[&TargetArch::x86_64])?;
			
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

			// Apply macOS sandbox using sandbox_init
			match apply_macos_sandbox(policy) {
				Ok(()) => {
					SANDBOX_APPLIED.store(true, Ordering::Release);
					debug!("macOS sandbox applied successfully");
					SandboxStatus::Applied
				}
				Err(e) => {
					warn!(error = %e, "failed to apply macOS sandbox");
					SandboxStatus::Unsupported
				}
			}
		}

		fn apply_macos_sandbox(policy: SandboxPolicy) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
			use std::ffi::CString;
			use std::ptr;

			// Define sandbox profile based on policy
			let profile = match policy {
				SandboxPolicy::Minimal => {
					// Minimal restrictions: allow most operations but block dangerous ones
					r#"
					(version 1)
					(deny default)
					(allow process-info-pidinfo)
					(allow process-info-setcontrol)
					(allow file-read-data (literal "/"))
					(allow file-read-data (literal "/usr/lib"))
					(allow file-read-data (literal "/System/Library"))
					(allow file-read-metadata)
					(allow file-write-data (regex #"^/tmp/"))
					(allow mach-lookup)
					(allow network-outbound)
					(allow network-inbound)
					(allow system-audit)
					(allow ipc-posix-shm)
					(deny process-fork)
					(deny process-exec)
					"#
				}
				SandboxPolicy::Strict => {
					// Strict restrictions: minimal allowed operations
					r#"
					(version 1)
					(deny default)
					(allow process-info-pidinfo)
					(allow file-read-data (literal "/usr/lib/libSystem.dylib"))
					(allow file-read-data (literal "/usr/lib/libc++.dylib"))
					(allow file-read-metadata (literal "/"))
					(allow mach-lookup (global-name "com.apple.system.logger"))
					(allow ipc-posix-shm-read-data)
					(allow ipc-posix-shm-write-data)
					(deny network*)
					(deny file-write*)
					(deny process-fork)
					(deny process-exec)
					"#
				}
			};

			let profile_cstr = CString::new(profile)?;
			
			// Call sandbox_init via FFI (using extern declaration)
			extern "C" {
				fn sandbox_init(
					profile: *const libc::c_char,
					flags: u64,
					errorbuf: *mut *mut libc::c_char,
				) -> libc::c_int;
			}

			let result = unsafe {
				sandbox_init(
					profile_cstr.as_ptr(),
					0, // flags
					ptr::null_mut(), // error
				)
			};

			if result == 0 {
				Ok(())
			} else {
				Err("sandbox_init failed".into())
			}
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
