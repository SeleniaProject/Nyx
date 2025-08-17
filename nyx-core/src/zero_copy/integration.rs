use super::manager::Buffer;

/// ゼロコピー: Buffer から &[u8] を借用（コピーなし）
pub fn into_bytes(b: &Buffer) -> &[u8] { b.as_slice() }

/// AEADなどに渡すためのバッファビュー（複数チャンクからなるケース向け）。
/// 今は単一Bufferをラップするが、将来 scatter/gather に拡張可能。
#[derive(Clone, Debug)]
pub struct ByteView<'a> {
	pub parts: Vec<&'a [u8]>,
}

impl<'a> From<&'a Buffer> for ByteView<'a> {
	fn from(b: &'a Buffer) -> Self { Self { parts: vec![b.as_slice()] } }
}

impl<'a> ByteView<'a> {
	pub fn len(&self) -> usize { self.parts.iter().map(|p| p.len()).sum() }
	pub fn is_empty(&self) -> bool { self.parts.iter().all(|p| p.is_empty()) }
}

/// 1280Bシャードにゼロコピーで区切るビュー（FEC最適化）
#[cfg(feature = "fec")]
pub mod fec_views {
	use super::*;
	use nyx_fec::padding::SHARD_SIZE;

	/// 入力を1280B境界で区切るスライスの配列（余りは最後だけ短い）。コピーしない。
	pub fn shard_view(buf: &Buffer) -> Vec<&[u8]> {
		let bytes = buf.as_slice();
		let mut v = Vec::with_capacity((bytes.len() + SHARD_SIZE - 1) / SHARD_SIZE);
		let mut i = 0;
		while i < bytes.len() {
			let end = (i + SHARD_SIZE).min(bytes.len());
			v.push(&bytes[i..end]);
			i = end;
		}
		v
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn passes_through() {
		let b = Buffer::from_vec(vec![1,2,3]);
		assert_eq!(into_bytes(&b), &[1,2,3]);
	}

	#[test]
	fn byteview_from_buffer() {
		let b = Buffer::from_vec((0u8..64).collect());
		let v: ByteView = (&b).into();
		assert_eq!(v.len(), 64);
		assert!(!v.is_empty());
		assert_eq!(v.parts.len(), 1);
		assert_eq!(v.parts[0].len(), 64);
	}
}

#[cfg(all(test, feature = "fec"))]
mod fec_tests {
	use super::*;
	use nyx_fec::{padding::SHARD_SIZE, rs1280::{Rs1280, RsConfig}};

	#[test]
	fn shard_view_and_parity_encode() {
		// 準備: 1.5シャード分のデータ
		let mut data = vec![0u8; SHARD_SIZE + SHARD_SIZE/2];
		for (i, b) in data.iter_mut().enumerate() { *b = (i % 251) as u8; }
		let buf: Buffer = data.into();

		// ゼロコピーでビューを作る
		let shards = fec_views::shard_view(&buf);
		assert_eq!(shards.len(), 2);
		assert_eq!(shards[0].len(), SHARD_SIZE);
		assert_eq!(shards[1].len(), SHARD_SIZE/2);

		// パリティ1枚（ゼロフルパディング扱い）を生成
		let cfg = RsConfig { data_shards: 2, parity_shards: 1 };
		let rs = Rs1280::new(cfg).unwrap();
		let d0: &[u8; SHARD_SIZE] = shards[0].try_into().unwrap();
		// 2枚目は短いので、一時的に埋めて固定参照にする（演算はin-placeなのでコピー最小）
		let mut tmp = [0u8; SHARD_SIZE];
		tmp[..shards[1].len()].copy_from_slice(shards[1]);
		let d1: &[u8; SHARD_SIZE] = &tmp;
		let mut p0 = [0u8; SHARD_SIZE];
		rs.encode_parity(&[d0, d1], &mut [&mut p0]).unwrap();
		// パリティが非ゼロになることを確認
		assert!(p0.iter().any(|&x| x != 0));
	}
}
