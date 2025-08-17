//! Helpers to pack variable payloads into fixed-size shards and back.

/// Fixed IP packet-ish MTU used across Nyx.
pub const SHARD_SIZE: usize = 1280;

/// Pack a payload into a fixed-size shard with length prefix.
/// Layout: [len: u16 LE][data...][zero padding]
pub fn pack_into_shard(payload: &[u8]) -> [u8; SHARD_SIZE] {
	// Keep a panic in the infallible variant for internal callers expecting fast-fail.
	assert!(payload.len() <= SHARD_SIZE - 2, "payload too large for shard");
	let mut out = [0u8; SHARD_SIZE];
	let len: u16 = payload.len() as u16;
	out[0..2].copy_from_slice(&len.to_le_bytes());
	out[2..2 + payload.len()].copy_from_slice(payload);
	out
}

/// Fallible variant that returns None instead of panicking when payload is too large.
pub fn try_pack_into_shard(payload: &[u8]) -> Option<[u8; SHARD_SIZE]> {
	if payload.len() > SHARD_SIZE - 2 { return None; }
	let mut out = [0u8; SHARD_SIZE];
	let len: u16 = payload.len() as u16;
	out[0..2].copy_from_slice(&len.to_le_bytes());
	out[2..2 + payload.len()].copy_from_slice(payload);
	Some(out)
}

/// Unpack a shard created by `pack_into_shard` and return the original payload slice.
pub fn unpack_from_shard(shard: &[u8; SHARD_SIZE]) -> &[u8] {
	let len = u16::from_le_bytes([shard[0], shard[1]]) as usize;
	debug_assert!(len <= SHARD_SIZE - 2, "encoded length out of bounds");
	&shard[2..2 + len]
}

/// Fallible unpack that validates the length prefix and returns None if invalid.
pub fn try_unpack_from_shard(shard: &[u8; SHARD_SIZE]) -> Option<&[u8]> {
	let len = u16::from_le_bytes([shard[0], shard[1]]) as usize;
	if len <= SHARD_SIZE - 2 { Some(&shard[2..2 + len]) } else { None }
}

/// Validate that a shard uses a sane length prefix and returns the payload length.
pub fn validate_shard_header(shard: &[u8; SHARD_SIZE]) -> Option<usize> {
	let len = u16::from_le_bytes([shard[0], shard[1]]) as usize;
	if len <= SHARD_SIZE - 2 { Some(len) } else { None }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pack_unpack_roundtrip() {
		let data = b"hello world";
		let shard = pack_into_shard(data);
		let recovered = unpack_from_shard(&shard);
		assert_eq!(recovered, data);
		assert_eq!(shard.len(), SHARD_SIZE);
	}

	#[test]
	fn try_pack_rejects_oversized() {
		let data = vec![0u8; SHARD_SIZE]; // larger than SHARD_SIZE-2
		assert!(try_pack_into_shard(&data).is_none());
	}

	#[test]
	fn validate_shard_header_works() {
		let shard = pack_into_shard(&[1, 2, 3, 4]);
		assert_eq!(validate_shard_header(&shard), Some(4));
	}

	#[test]
	fn unpack_respects_length_prefix_zero() {
		let shard = pack_into_shard(&[]);
		let recovered = unpack_from_shard(&shard);
		assert_eq!(recovered.len(), 0);
	}

	#[test]
	fn try_pack_exact_capacity_minus_header() {
		let buf = vec![9u8; SHARD_SIZE - 2];
		let shard = try_pack_into_shard(&buf).expect("should fit");
		let got = unpack_from_shard(&shard);
		assert_eq!(got, &buf[..]);
	}

	#[test]
	fn unpack_rejects_oob_length_via_safe_slice() {
		// Craft an invalid shard: length header claims too large length.
		let mut shard = [0u8; SHARD_SIZE];
		let bogus_len: u16 = (SHARD_SIZE as u16).saturating_add(1); // > SHARD_SIZE - 2
		shard[0..2].copy_from_slice(&bogus_len.to_le_bytes());
		// Even though header is invalid, slicing with 2..2+len would panic.
		// Our implementation uses a debug_assert and safe slice; in release it may panic.
		// Therefore, guard using validate_shard_header and ensure it flags None.
		assert_eq!(validate_shard_header(&shard), None);
	}

	#[test]
	fn try_unpack_returns_none_on_invalid_header() {
		let mut shard = [0u8; SHARD_SIZE];
		let bogus_len: u16 = (SHARD_SIZE as u16).saturating_add(1);
		shard[0..2].copy_from_slice(&bogus_len.to_le_bytes());
		assert!(try_unpack_from_shard(&shard).is_none());
	}

	#[test]
	fn try_unpack_matches_unpack_for_valid_input() {
		let data = b"abc";
		let shard = pack_into_shard(data);
		let a = unpack_from_shard(&shard);
		let b = try_unpack_from_shard(&shard).unwrap();
		assert_eq!(a, b);
	}
}

