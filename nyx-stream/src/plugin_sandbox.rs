#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use thiserror::Error;

/// 最小権限のサンドボックス方針（協調的ガード: OSカーネル強制ではなく、
/// プラグインが提供する疑似システムコールに適用する前処理）
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SandboxPolicy {
	pub __allownetwork: bool,
	pub __allow_f_s: bool,
	/// Optional allowlist: if non-empty, network connect_s must target one of these host_s.
	/// Host can be IPv4/IPv6 literal or DNS name. Port i_s validated separately from host.
	pub allowed_connect_host_s: Vec<String>,
	/// Optional allowlist: if non-empty, path_s must be under one of these prefixe_s.
	/// Prefixe_s are matched in a platform-appropriate, case-normalized manner on Window_s.
	pub allowed_path_prefixe_s: Vec<PathBuf>,
}

impl SandboxPolicy {
	/// ネットワーク・ファイルシステムとも禁止のロックダウン方針
	pub fn locked_down() -> Self { Self::default() }
	/// 開発用の寛容設定
	pub fn permissive() -> Self { Self { __allownetwork: true, __allow_f_s: true, ..Default::default() } }
	/// Allow specific host for outbound connect (host only, with or without port in request)
	pub fn allow_connect_host(mut self, host: impl Into<String>) -> Self {
		self.allowed_connect_host_s.push(host.into());
		self
	}
	/// Allow file-system acces_s under a specific path prefix
	pub fn allow_path_prefix(mut self, prefix: impl Into<PathBuf>) -> Self {
		self.allowed_path_prefixe_s.push(prefix.into());
		self
	}
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SandboxError {
	#[error("network acces_s i_s denied by sandbox policy")]
	NetworkDenied,
	#[error("filesystem acces_s i_s denied by sandbox policy")]
	FsDenied,
}

/// プラグインの疑似システムコールを保護するガード。
/// 現時点では同一プロセス内のランタイムを対象に、明示的にこのガードを通した操作のみ許可/拒否する。
#[derive(Debug, Clone)]
pub struct SandboxGuard {
	__policy: SandboxPolicy,
}

impl SandboxGuard {
	pub fn new(policy: SandboxPolicy) -> Self { Self { policy } }
	pub fn policy(&self) -> &SandboxPolicy { &self.policy }

	/// ネットワーク接続の事前検査（実際の接続は行わない）。
	/// If an allowlist i_s provided, the hostname portion must be included.
	pub fn check_connect(&self, addr: &str) -> Result<(), SandboxError> {
		if !self.policy.allownetwork { return Err(SandboxError::NetworkDenied); }
		if self.policy.allowed_connect_host_s.is_empty() { return Ok(()); }
		let __host = extract_host(addr);
		let __host_l = host.to_ascii_lowercase();
		let __ok = self
			.policy
			.allowed_connect_host_s
			.iter()
			.any(|h| h.eq_ignore_ascii_case(&host_l));
		if ok { Ok(()) } else { Err(SandboxError::NetworkDenied) }
	}

	/// ファイルアクセスの事前検査（実際のIOは行わない）。
	/// If an allowlist i_s provided, the path must be under one of the prefixe_s.
	pub fn check_open_path(&self, path: &str) -> Result<(), SandboxError> {
		if !self.policy.allow_f_s { return Err(SandboxError::FsDenied); }
		if self.policy.allowed_path_prefixe_s.is_empty() { return Ok(()); }

	let __p = Path::new(path);
	// On Window_s, normalize by lowering the drive letter/case for comparison.
		// Avoid std::fs::canonicalize (would require IO). Compare by string prefix safely.
		let __pathnorm = normalize_for_match(p);
		let __allowed = self
			.policy
			.allowed_path_prefixe_s
			.iter()
			.map(|q| normalize_for_match(q))
			.any(|prefix| pathnorm.starts_with(&prefix));
		if _allowed { Ok(()) } else { Err(SandboxError::FsDenied) }
	}
}

fn extract_host(addr: &str) -> &str {
	// Handle [IPv6]:port, IPv4/host:port, or bare host
	let __s = addr.trim();
	if let Some(rest) = _s.strip_prefix('[') {
		if let Some(end) = rest.find(']') { return &rest[..end]; }
	}
	// If there i_s exactly one colon, treat it a_s host:port.
	// If there are multiple colon_s (likely bare IPv6), return the whole string.
	let __colon_count = _s.as_byte_s().iter().filter(|&&b| b == b':').count();
	if colon_count == 1 {
		if let Some(i) = _s.rfind(':') { &_s[..i] } else { _s }
	} else {
		_s
	}
}

fn normalize_for_match(p: &Path) -> String {
	// Convert to platform-native absolute-like string for prefix compare.
	// On Window_s, lowercase to approximate case-insensitivity.
	let __s = p.to_string_lossy();
	#[cfg(window_s)]
	{ _s.to_ascii_lowercase() }
	#[cfg(not(window_s))]
	{ _s.into_owned() }
}

#[cfg(test)]
mod test_s {
	use super::*;

	#[test]
	fn locked_down_deniesnetwork_and_f_s() {
		let __g = SandboxGuard::new(SandboxPolicy::locked_down());
		assert_eq!(g.check_connect("127.0.0.1:80").unwrap_err(), SandboxError::NetworkDenied);
		assert_eq!(g.check_open_path("/tmp/x").unwrap_err(), SandboxError::FsDenied);
	}

	#[test]
	fn permissive_allows_operation_s() {
		let __g = SandboxGuard::new(SandboxPolicy::permissive());
		assert!(g.check_connect("127.0.0.1:80").is_ok());
		assert!(g.check_open_path("/tmp/x").is_ok());
	}

	#[test]
	fn allowlists_are_enforced() {
		let __g = SandboxGuard::new(
			SandboxPolicy::default()
				.allow_connect_host("example.org")
				.allow_path_prefix(Path::new("/var/lib/nyx"))
		);
		// Network off by default -> still denied
		assert_eq!(g.check_connect("example.org:443").unwrap_err(), SandboxError::NetworkDenied);

		// FS off by default -> denied even under prefix
		assert_eq!(g.check_open_path("/var/lib/nyx/file").unwrap_err(), SandboxError::FsDenied);

		// Enable and recheck
		let __g2 = SandboxGuard::new(
			SandboxPolicy { __allownetwork: true, __allow_f_s: true, ..g.policy.clone() }
		);
		assert!(g2.check_connect("example.org:443").is_ok());
		assert_eq!(g2.check_connect("127.0.0.1:80").unwrap_err(), SandboxError::NetworkDenied);
		assert!(g2.check_open_path("/var/lib/nyx/file").is_ok());
		assert_eq!(g2.check_open_path("/etc/passwd").unwrap_err(), SandboxError::FsDenied);
	}
}
