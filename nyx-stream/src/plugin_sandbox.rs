#![forbid(unsafe_code)]

use thiserror::Error;

/// 最小権限のサンドボックス方針
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SandboxPolicy {
	pub allow_network: bool,
	pub allow_fs: bool,
}

impl SandboxPolicy {
	/// ネットワーク・ファイルシステムとも禁止のロックダウン方針
	pub fn locked_down() -> Self { Self { allow_network: false, allow_fs: false } }
	/// 開発用の寛容設定
	pub fn permissive() -> Self { Self { allow_network: true, allow_fs: true } }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SandboxError {
	#[error("network access is denied by sandbox policy")]
	NetworkDenied,
	#[error("filesystem access is denied by sandbox policy")]
	FsDenied,
}

/// プラグインの疑似システムコールを保護するガード。
/// 現時点では同一プロセス内のランタイムを対象に、明示的にこのガードを通した操作のみ許可/拒否する。
#[derive(Debug, Clone)]
pub struct SandboxGuard {
	policy: SandboxPolicy,
}

impl SandboxGuard {
	pub fn new(policy: SandboxPolicy) -> Self { Self { policy } }
	pub fn policy(&self) -> &SandboxPolicy { &self.policy }

	/// ネットワーク接続の事前検査（実際の接続は行わない）
	pub fn check_connect(&self, _addr: &str) -> Result<(), SandboxError> {
		if self.policy.allow_network { Ok(()) } else { Err(SandboxError::NetworkDenied) }
	}

	/// ファイルアクセスの事前検査（実際のIOは行わない）
	pub fn check_open_path(&self, _path: &str) -> Result<(), SandboxError> {
		if self.policy.allow_fs { Ok(()) } else { Err(SandboxError::FsDenied) }
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn locked_down_denies_network_and_fs() {
		let g = SandboxGuard::new(SandboxPolicy::locked_down());
		assert_eq!(g.check_connect("127.0.0.1:80").unwrap_err(), SandboxError::NetworkDenied);
		assert_eq!(g.check_open_path("/tmp/x").unwrap_err(), SandboxError::FsDenied);
	}

	#[test]
	fn permissive_allows_operations() {
		let g = SandboxGuard::new(SandboxPolicy::permissive());
		assert!(g.check_connect("127.0.0.1:80").is_ok());
		assert!(g.check_open_path("/tmp/x").is_ok());
	}
}
