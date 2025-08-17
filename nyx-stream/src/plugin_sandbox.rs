#![forbid(unsafe_code)]

/// Sandbox policy placeholder. In production builds, a platform-specific
/// implementation should restrict file system and network.
#[derive(Debug, Clone, Default)]
pub struct SandboxPolicy {
	pub allow_network: bool,
	pub allow_fs: bool,
}

impl SandboxPolicy {
	pub fn locked_down() -> Self { Self { allow_network: false, allow_fs: false } }
}
