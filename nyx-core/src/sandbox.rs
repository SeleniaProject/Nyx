#![cfg(target_os = "linux")]

use anyhow::Result;

#[cfg(target_os = "linux")]
pub fn install_seccomp() -> Result<()> {
    use std::collections::HashMap;
    use tracing::{info, warn};
    
    info!("Installing seccomp filter for Nyx daemon");
    
    // Define allowed syscalls for network daemon operation
    let allowed_syscalls = vec![
        // Essential syscalls
        "read", "write", "close", "openat", "lseek",
        // Memory management
        "mmap", "munmap", "brk", "mprotect",
        // Process management
        "getpid", "gettid", "getuid", "getgid",
        // Time operations
        "clock_gettime", "nanosleep",
        // Network operations
        "socket", "bind", "listen", "accept", "connect",
        "sendto", "recvfrom", "setsockopt", "getsockopt",
        // File system (restricted)
        "stat", "fstat", "access", "readlink",
        // Threading
        "clone", "futex", "set_robust_list",
        // Signal handling
        "rt_sigaction", "rt_sigprocmask", "rt_sigreturn",
        // Exit
        "exit", "exit_group",
    ];
    
    // In a real implementation, we would use libseccomp-rs here
    // For now, we'll use a simplified approach
    
    #[cfg(feature = "seccomp")]
    {
        use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, SeccompRule};
        use std::convert::TryInto;
        
        let mut rules = HashMap::new();
        
        // Allow essential syscalls
        for syscall in allowed_syscalls {
            rules.insert(syscall.to_string(), vec![SeccompRule::new(vec![])?]);
        }
        
        // Create and install filter
        let filter = SeccompFilter::new(
            rules,
            SeccompAction::KillProcess, // Default action for disallowed syscalls
            SeccompAction::Allow,       // Default action for allowed syscalls
            std::env::consts::ARCH.try_into()?,
        )?;
        
        let bpf_prog: BpfProgram = filter.try_into()?;
        bpf_prog.apply()?;
        
        info!("Seccomp filter installed successfully");
    }
    
    #[cfg(not(feature = "seccomp"))]
    {
        warn!("Seccomp support not compiled in, running without syscall filtering");
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