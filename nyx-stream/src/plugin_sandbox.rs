#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use thiserror::Error;

/// 最小権限のサンドボックス方針（協調的ガード: OSカーネル強制ではなく、
/// プラグインが提供する疑似システムコールに適用する前処理）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxPolicy {
	pub allow_network: bool,
	pub allow_fs: bool,
	/// Optional allowlist: if non-empty, network connects must target one of these hosts.
	/// Host can be IPv4/IPv6 literal or DNS name. Port is validated separately from host.
	pub allowed_connect_hosts: Vec<String>,
	/// Optional allowlist: if non-empty, paths must be under one of these prefixes.
	/// Prefixes are matched in a platform-appropriate, case-normalized manner on Windows.
	pub allowed_path_prefixes: Vec<PathBuf>,
}

impl Default for SandboxPolicy {
	fn default() -> Self {
		Self {
			allow_network: false,
			allow_fs: false,
			allowed_connect_hosts: Vec::new(),
			allowed_path_prefixes: Vec::new(),
		}
	}
}

impl SandboxPolicy {
	/// ネットワーク・ファイルシステムとも禁止のロックダウン方針
	pub fn locked_down() -> Self { Self::default() }
	/// 開発用の寛容設定
	pub fn permissive() -> Self { Self { allow_network: true, allow_fs: true, ..Default::default() } }
	/// Allow specific host for outbound connect (host only, with or without port in request)
	pub fn allow_connect_host(mut self, host: impl Into<String>) -> Self {
		self.allowed_connect_hosts.push(host.into());
		self
	}
	/// Allow file-system access under a specific path prefix
	pub fn allow_path_prefix(mut self, prefix: impl Into<PathBuf>) -> Self {
		self.allowed_path_prefixes.push(prefix.into());
		self
	}
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

	/// ネットワーク接続の事前検査（実際の接続は行わない）。
	/// If an allowlist is provided, the hostname portion must be included.
	pub fn check_connect(&self, addr: &str) -> Result<(), SandboxError> {
		if !self.policy.allow_network { return Err(SandboxError::NetworkDenied); }
		if self.policy.allowed_connect_hosts.is_empty() { return Ok(()); }
		let host = extract_host(addr);
		let host_l = host.to_ascii_lowercase();
		let ok = self
			.policy
			.allowed_connect_hosts
			.iter()
			.any(|h| h.eq_ignore_ascii_case(&host_l));
		if ok { Ok(()) } else { Err(SandboxError::NetworkDenied) }
	}

	/// ファイルアクセスの事前検査（実際のIOは行わない）。
	/// If an allowlist is provided, the path must be under one of the prefixes.
	pub fn check_open_path(&self, path: &str) -> Result<(), SandboxError> {
		if !self.policy.allow_fs { return Err(SandboxError::FsDenied); }
		if self.policy.allowed_path_prefixes.is_empty() { return Ok(()); }

	let p = Path::new(path);
	// On Windows, normalize by lowering the drive letter/case for comparison.
		// Avoid std::fs::canonicalize (would require IO). Compare by string prefix safely.
		let path_norm = normalize_for_match(p);
		let allowed = self
			.policy
			.allowed_path_prefixes
			.iter()
			.map(|q| normalize_for_match(q))
			.any(|prefix| path_norm.starts_with(&prefix));
		if allowed { Ok(()) } else { Err(SandboxError::FsDenied) }
	}
}

fn extract_host(addr: &str) -> &str {
	// Handle [IPv6]:port, IPv4/host:port, or bare host
	let s = addr.trim();
	if let Some(rest) = s.strip_prefix('[') {
		if let Some(end) = rest.find(']') { return &rest[..end]; }
	}
	// Use last ':' to split host:port (to keep IPv6 without brackets from being mis-parsed)
	if let Some(i) = s.rfind(':') { &s[..i] } else { s }
}

fn normalize_for_match(p: &Path) -> String {
	// Convert to platform-native absolute-like string for prefix compare.
	// On Windows, lowercase to approximate case-insensitivity.
	let s = p.to_string_lossy();
	#[cfg(windows)]
	{ s.to_ascii_lowercase() }
	#[cfg(not(windows))]
	{ s.into_owned() }
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

	#[test]
	fn allowlists_are_enforced() {
		let g = SandboxGuard::new(
			SandboxPolicy::default()
				.allow_connect_host("example.org")
				.allow_path_prefix(Path::new("/var/lib/nyx"))
		);
		// Network off by default -> still denied
		assert_eq!(g.check_connect("example.org:443").unwrap_err(), SandboxError::NetworkDenied);

		// FS off by default -> denied even under prefix
		assert_eq!(g.check_open_path("/var/lib/nyx/file").unwrap_err(), SandboxError::FsDenied);

		// Enable and recheck
		let g2 = SandboxGuard::new(
			SandboxPolicy { allow_network: true, allow_fs: true, ..g.policy.clone() }
		);
		assert!(g2.check_connect("example.org:443").is_ok());
		assert_eq!(g2.check_connect("127.0.0.1:80").unwrap_err(), SandboxError::NetworkDenied);
		assert!(g2.check_open_path("/var/lib/nyx/file").is_ok());
		assert_eq!(g2.check_open_path("/etc/passwd").unwrap_err(), SandboxError::FsDenied);
	}
}
