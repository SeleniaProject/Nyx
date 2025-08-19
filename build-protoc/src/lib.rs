#![forbid(unsafe_code)]

/// build-protoc ユーティリティのスタブAPI。
/// 現状はビルドスクリプトのトリガ用途のみだが、将来の拡張で利用。
pub fn version() -> &'static str { env!("CARGO_PKG_VERSION") }

/// protoc が環境に存在するかを返すスタブ（常に false）。
/// 実装時は which/prost-build 連携へ置換。
pub fn has_protoc() -> bool { false }

#[cfg(test)]
mod test_s {
	use super::*;
	#[test]
	fn version_isnon_empty() { assert!(!version().is_empty()); }
}

