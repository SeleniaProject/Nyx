use super::manager::Buffer;

/// Stub for integrating zero-copy buffers with crypto/FEC layers.
pub fn into_bytes(b: &Buffer) -> &[u8] { b.as_slice() }

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn passes_through() {
		let b = Buffer::from_vec(vec![1,2,3]);
		assert_eq!(into_bytes(&b), &[1,2,3]);
	}
}
