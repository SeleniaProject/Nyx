/// Cross-platform sandbox policy stub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxPolicy { Minimal, Strict }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxStatus { Applied, Unsupported }

pub fn apply_policy(_p: SandboxPolicy) -> SandboxStatus {
	// In core, keep pure Rust and no-ops. OS-specific enforcers live elsewhere.
	SandboxStatus::Unsupported
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn sandbox_stub() { assert_eq!(apply_policy(SandboxPolicy::Minimal), SandboxStatus::Unsupported); }
}
