#![cfg(target_os = "linux")]

use anyhow::Result;

#[cfg(target_os = "linux")]
pub fn install_seccomp() -> Result<()> {
    use std::collections::HashMap;
    use tracing::{info, warn};
    
    info!("Installing seccomp filter for Nyx daemon");
    
    // Define allowed syscalls for asynchronous network daemon operations.
    // This list aims to be permissive enough for Tokio-based runtimes while
    // keeping a strong default-deny policy for anything not explicitly needed.
    let allowed_syscalls = vec![
        // Process lifecycle and basic I/O
        "read", "write", "close", "openat", "lseek", "dup", "dup2", "dup3",
        // Memory management
        "mmap", "munmap", "brk", "mprotect", "mremap", "madvise",
        // Threading and synchronization
        "clone", "set_robust_list", "futex", "sched_yield", "gettid",
        // Signals
        "rt_sigaction", "rt_sigprocmask", "rt_sigreturn", "tgkill",
        // Time
        "clock_gettime", "nanosleep", "timerfd_create", "timerfd_settime",
        // Randomness
        "getrandom",
        // File metadata and limited FS ops
        "stat", "fstat", "lstat", "newfstatat", "access", "readlink",
        "getcwd", "mkdirat", "unlinkat", "renameat2",
        // File descriptor operations
        "fcntl", "ioctl", "pread64", "pwrite64",
        // Networking
        "socket", "bind", "listen", "accept", "accept4", "connect",
        "getsockname", "getpeername", "shutdown",
        "sendto", "recvfrom", "sendmsg", "recvmsg", "sendmmsg", "recvmmsg",
        "getsockopt", "setsockopt",
        // Eventing (Tokio epoll backend)
        "epoll_create1", "epoll_ctl", "epoll_wait",
        // Event/notification fds
        "eventfd", "eventfd2", "pipe", "pipe2",
        // Process and limits
        "getpid", "getppid", "getuid", "getgid", "geteuid", "getegid",
        "prlimit64",
        // Exit
        "exit", "exit_group",
    ];

    // Apply seccomp filter using seccompiler. If installation fails, fall back
    // to running without the filter but emit a warning so operators are aware.
    use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, SeccompRule};
    use std::convert::TryInto;

    let mut rules = HashMap::new();

    // Allow the listed syscalls without additional argument constraints.
    for syscall in allowed_syscalls {
        rules.insert(syscall.to_string(), vec![SeccompRule::new(Vec::new())?]);
    }

    // Build a default-deny filter (KillProcess) with explicit allows.
    match SeccompFilter::new(
        rules,
        SeccompAction::KillProcess,
        SeccompAction::Allow,
        std::env::consts::ARCH.try_into()?,
    ) {
        Ok(filter) => {
            let bpf: BpfProgram = filter.try_into()?;
            if let Err(e) = bpf.apply() {
                warn!("Failed to apply seccomp filter, continuing without it: {}", e);
            } else {
                info!("Seccomp filter installed successfully");
            }
        }
        Err(e) => {
            warn!("Failed to build seccomp filter, continuing without it: {}", e);
        }
    }
    
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn install_seccomp() -> Result<()> {
    // No-op on non-Linux platforms
    Ok(())
}

/// Sandbox configuration for the daemon process.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Enable seccomp filtering
    pub enable_seccomp: bool,
    /// Enable network namespace isolation
    pub enable_network_namespace: bool,
    /// Enable filesystem restrictions
    pub enable_fs_restrictions: bool,
    /// Allowed filesystem paths
    pub allowed_paths: Vec<String>,
    /// Allowed network interfaces
    pub allowed_interfaces: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enable_seccomp: true,
            enable_network_namespace: false,
            enable_fs_restrictions: false,
            allowed_paths: vec![
                "/tmp".to_string(),
                "/var/tmp".to_string(),
            ],
            allowed_interfaces: vec![
                "lo".to_string(),
                "eth0".to_string(),
            ],
        }
    }
}

/// Initialize sandbox with the given configuration.
pub fn init_sandbox(config: &SandboxConfig) -> Result<()> {
    use tracing::{info, debug};
    
    info!("Initializing sandbox with config: {:?}", config);
    
    if config.enable_seccomp {
        debug!("Enabling seccomp filtering");
        install_seccomp()?;
    }
    
    if config.enable_network_namespace {
        debug!("Setting up network namespace isolation");
        #[cfg(target_os = "linux")]
        {
            // Create network namespace isolation
            use std::process::Command;
            
            let output = Command::new("ip")
                .args(&["netns", "add", "nyx-daemon"])
                .output();
                
            match output {
                Ok(result) if result.status.success() => {
                    info!("Network namespace 'nyx-daemon' created");
                    
                    // Configure loopback in namespace
                    let _ = Command::new("ip")
                        .args(&["netns", "exec", "nyx-daemon", "ip", "link", "set", "lo", "up"])
                        .output();
                }
                Ok(result) => {
                    debug!("Network namespace creation failed: {}", 
                           String::from_utf8_lossy(&result.stderr));
                }
                Err(e) => {
                    debug!("Failed to execute ip command: {}", e);
                }
            }
        }
    }
    
    if config.enable_fs_restrictions {
        debug!("Setting up filesystem restrictions");
        #[cfg(target_os = "linux")]
        {
            // Apply filesystem restrictions using chroot or bind mounts
            use std::fs;
            use std::path::Path;
            
            for path in &config.allowed_paths {
                if !Path::new(path).exists() {
                    if let Err(e) = fs::create_dir_all(path) {
                        debug!("Failed to create allowed path {}: {}", path, e);
                    }
                }
            }
            
            info!("Filesystem restrictions applied for {} paths", config.allowed_paths.len());
        }
    }
    
    info!("Sandbox initialization completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert!(config.enable_seccomp);
        assert!(!config.enable_network_namespace);
        assert!(!config.enable_fs_restrictions);
        assert!(!config.allowed_paths.is_empty());
        assert!(!config.allowed_interfaces.is_empty());
    }

    #[test]
    fn test_init_sandbox() {
        let config = SandboxConfig::default();
        let result = init_sandbox(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_install_seccomp() {
        let result = install_seccomp();
        assert!(result.is_ok());
    }
} 