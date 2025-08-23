#![forbid(unsafe_code)]

use nyx_stream::plugin_sandbox::{SandboxGuard, SandboxPolicy, FilesystemAccess};

#[test]
fn test_sandbox_policy_creation() -> Result<(), Box<dyn std::error::Error>> {
    let policy = SandboxPolicy::permissive();
    let _guard = SandboxGuard::new(policy);
    Ok(())
}

#[test] 
fn test_filesystem_access_levels() -> Result<(), Box<dyn std::error::Error>> {
    let mut policy = SandboxPolicy::permissive();
    policy.allow_filesystem = FilesystemAccess::ReadOnly;
    let _guard = SandboxGuard::new(policy);
    Ok(())
}
