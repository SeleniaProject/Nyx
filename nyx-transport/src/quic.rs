//! QUIC transport (feature-gated stub). Real QUIC impl is disabled to avoid C deps.

/// Returns false; real QUIC requires external crates with C deps (ring).
pub fn is_supported() -> bool { cfg!(feature = "quic") }

#[cfg(test)]
mod tests {
	#[test]
	fn feature_flag_reflects() { assert_eq!(super::is_supported(), cfg!(feature = "quic")); }
}
