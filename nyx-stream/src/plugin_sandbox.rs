#![forbid(unsafe_code)]

//! Plugin sandbox system for secure runtime isolation
//! Cooperative policy enforcement through resource access controls.

use std::path::Path;
use thiserror::Error;
use tracing::{debug, warn};

/// Sandbox policy configuration
#[derive(Debug, Clone)]
pub struct SandboxPolicy {
    /// Allow network access
    pub allow_network: bool,
    /// Allow file system access (read-only or full)
    pub allow_filesystem: FilesystemAccess,
    /// Maximum memory usage (in bytes)
    pub memory_limit: Option<usize>,
    /// Allowed network destinations (CIDR or domain patterns)
    pub network_allowlist: Vec<String>,
    /// Allowed filesystem paths
    pub filesystem_allowlist: Vec<String>,
}

/// Filesystem access levels
#[derive(Debug, Clone, Default)]
pub enum FilesystemAccess {
    #[default]
    None,
    ReadOnly,
    Full,
}

/// Sandbox enforcement errors
#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("network access is denied by sandbox policy")]
    NetworkAccessDenied,
    #[error("file access is denied by sandbox policy: {0}")]
    FileAccessDenied(String),
    #[error("memory limit exceeded: {0} bytes")]
    MemoryLimitExceeded(usize),
    #[error("sandbox configuration error: {0}")]
    ConfigurationError(String),
}

/// Runtime sandbox guard - enforces policy during plugin execution
#[derive(Debug, Clone)]
pub struct SandboxGuard {
    policy: SandboxPolicy,
}

impl SandboxGuard {
    /// Create a new sandbox guard with the given policy
    pub fn new(policy: SandboxPolicy) -> Self {
        Self { policy }
    }

    /// Check if network connection to destination is allowed
    pub fn check_connect(&self, destination: &str) -> Result<(), SandboxError> {
        if !self.policy.allow_network {
            warn!(destination = %destination, "Network access denied by sandbox");
            return Err(SandboxError::NetworkAccessDenied);
        }

        // If allowlist is empty, allow all connections
        if self.policy.network_allowlist.is_empty() {
            debug!(destination = %destination, "Network access granted (no restrictions)");
            return Ok(());
        }

        // Check against allowlist patterns
        for pattern in &self.policy.network_allowlist {
            if destination_matches_pattern(destination, pattern) {
                debug!(destination = %destination, pattern = %pattern, "Network access granted");
                return Ok(());
            }
        }

        warn!(destination = %destination, "Network access denied - not in allowlist");
        Err(SandboxError::NetworkAccessDenied)
    }

    /// Check if file path access is allowed
    pub fn check_open_path(&self, path: &str) -> Result<(), SandboxError> {
        match self.policy.allow_filesystem {
            FilesystemAccess::None => {
                warn!(path = %path, "File access denied by sandbox");
                Err(SandboxError::FileAccessDenied(path.to_string()))
            }
            FilesystemAccess::ReadOnly | FilesystemAccess::Full => {
                // Check allowlist if configured
                if !self.policy.filesystem_allowlist.is_empty() {
                    let normalized_path = Path::new(path).canonicalize().map_err(|_| {
                        SandboxError::FileAccessDenied(format!("Invalid path: {path}"))
                    })?;

                    for allowed_path in &self.policy.filesystem_allowlist {
                        let allowed_normalized =
                            Path::new(allowed_path).canonicalize().map_err(|_| {
                                SandboxError::ConfigurationError(format!(
                                    "Invalid allowlist path: {allowed_path}"
                                ))
                            })?;

                        if normalized_path.starts_with(&allowed_normalized) {
                            debug!(path = %path, "File access granted");
                            return Ok(());
                        }
                    }

                    warn!(path = %path, "File access denied - not in allowlist");
                    return Err(SandboxError::FileAccessDenied(path.to_string()));
                }

                debug!(path = %path, "File access granted (no restrictions)");
                Ok(())
            }
        }
    }

    /// Check memory usage against policy limits
    pub fn check_memory_usage(&self, current_usage: usize) -> Result<(), SandboxError> {
        if let Some(limit) = self.policy.memory_limit {
            if current_usage > limit {
                warn!(
                    usage = current_usage,
                    limit = limit,
                    "Memory limit exceeded"
                );
                return Err(SandboxError::MemoryLimitExceeded(current_usage));
            }
        }
        Ok(())
    }

    /// Get the current policy (read-only access)
    pub fn policy(&self) -> &SandboxPolicy {
        &self.policy
    }
}

/// Simple pattern matching for network destinations
/// Supports basic wildcard patterns and domain suffix matching
fn destination_matches_pattern(destination: &str, pattern: &str) -> bool {
    // Exact match
    if destination == pattern {
        return true;
    }

    // Simple wildcard patterns (only at beginning or end)
    if pattern.starts_with('*') && pattern.len() > 1 {
        let suffix = &pattern[1..];
        return destination.ends_with(suffix);
    }

    if pattern.ends_with('*') && pattern.len() > 1 {
        let prefix = &pattern[..pattern.len() - 1];
        return destination.starts_with(prefix);
    }

    // Domain suffix matching
    if pattern.starts_with('.') && destination.ends_with(pattern) {
        return true;
    }

    false
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            allow_network: false,
            allow_filesystem: FilesystemAccess::None,
            memory_limit: Some(64 * 1024 * 1024), // 64MB default
            network_allowlist: Vec::new(),
            filesystem_allowlist: Vec::new(),
        }
    }
}

/// Predefined sandbox policies for common use cases
impl SandboxPolicy {
    /// Strict policy: no network, no filesystem, limited memory
    pub fn strict() -> Self {
        Self {
            allow_network: false,
            allow_filesystem: FilesystemAccess::None,
            memory_limit: Some(32 * 1024 * 1024), // 32MB
            network_allowlist: Vec::new(),
            filesystem_allowlist: Vec::new(),
        }
    }

    /// Permissive policy: allows controlled access
    pub fn permissive() -> Self {
        Self {
            allow_network: true,
            allow_filesystem: FilesystemAccess::ReadOnly,
            memory_limit: Some(128 * 1024 * 1024), // 128MB
            network_allowlist: Vec::new(),         // Empty = allow all
            filesystem_allowlist: Vec::new(),      // Empty = allow all
        }
    }

    /// Allow access to a specific path prefix
    pub fn allow_path_prefix(mut self, path: &Path) -> Self {
        self.filesystem_allowlist
            .push(path.to_string_lossy().to_string());
        self
    }

    /// Allow connections to a specific host
    pub fn allow_connect_host(mut self, host: &str) -> Self {
        self.network_allowlist.push(host.to_string());
        self
    }

    /// Custom policy builder
    pub fn builder() -> SandboxPolicyBuilder {
        SandboxPolicyBuilder::default()
    }
}

/// Builder for creating custom sandbox policies
#[derive(Debug, Default)]
pub struct SandboxPolicyBuilder {
    policy: SandboxPolicy,
}

impl SandboxPolicyBuilder {
    pub fn allow_network(mut self, allow: bool) -> Self {
        self.policy.allow_network = allow;
        self
    }

    pub fn filesystem_access(mut self, access: FilesystemAccess) -> Self {
        self.policy.allow_filesystem = access;
        self
    }

    pub fn memory_limit(mut self, limit: Option<usize>) -> Self {
        self.policy.memory_limit = limit;
        self
    }

    pub fn add_network_allowlist<S: Into<String>>(mut self, pattern: S) -> Self {
        self.policy.network_allowlist.push(pattern.into());
        self
    }

    pub fn add_filesystem_allowlist<S: Into<String>>(mut self, path: S) -> Self {
        self.policy.filesystem_allowlist.push(path.into());
        self
    }

    pub fn build(self) -> SandboxPolicy {
        self.policy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_policy_denies_everything() {
        let guard = SandboxGuard::new(SandboxPolicy::strict());

        assert!(guard.check_connect("example.com").is_err());
        assert!(guard.check_open_path("/tmp/test.txt").is_err());
        assert!(guard.check_memory_usage(64 * 1024 * 1024).is_err()); // Over 32MB limit
    }

    #[test]
    fn test_permissive_policy_allows_access() {
        let guard = SandboxGuard::new(SandboxPolicy::permissive());

        assert!(guard.check_connect("example.com").is_ok());
        assert!(guard.check_memory_usage(64 * 1024 * 1024).is_ok()); // Under 128MB limit
    }

    #[test]
    fn test_network_allowlist() {
        let policy = SandboxPolicy::builder()
            .allow_network(true)
            .add_network_allowlist("example.com")
            .add_network_allowlist("*.trusted.org")
            .build();
        let guard = SandboxGuard::new(policy);

        assert!(guard.check_connect("example.com").is_ok());
        assert!(guard.check_connect("api.trusted.org").is_ok());
        assert!(guard.check_connect("malicious.com").is_err());
    }

    #[test]
    fn test_pattern_matching() {
        assert!(destination_matches_pattern("example.com", "example.com"));
        assert!(destination_matches_pattern(
            "api.example.com",
            "*.example.com"
        ));
        assert!(destination_matches_pattern(
            "api.example.com",
            ".example.com"
        ));
        assert!(!destination_matches_pattern("evil.com", "*.example.com"));
    }
}
