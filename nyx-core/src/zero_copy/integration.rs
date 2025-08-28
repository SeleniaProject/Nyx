use super::manager::Buffer;

/// Zero-copy: Buffer から &[u8] への軽量変換（コピーなし）
pub fn into_bytes(b: &Buffer) -> &[u8] {
    b.as_slice()
}

/// AEADなどに渡すためのバッファビュー（借用でリークしないケースのみ）。
/// 現在は単一Bufferをラップするが、将来 scatter/gather に拡張可能。
#[derive(Clone, Debug)]
pub struct ByteView<'a> {
    pub parts: Vec<&'a [u8]>,
}

impl<'a> ByteView<'a> {
    /// 単一バッファからビューを作成
    pub fn single(b: &'a Buffer) -> Self {
        Self {
            parts: vec![b.as_slice()],
        }
    }

    /// 複数バッファから scatter/gather ビューを作成
    pub fn multi(parts: Vec<&'a [u8]>) -> Self {
        Self { parts }
    }

    /// 全体のサイズを計算
    pub fn total_len(&self) -> usize {
        self.parts.iter().map(|p| p.len()).sum()
    }

    /// 線形化が必要な場合の緊急時コピー（避けるべき）
    pub fn to_vec(&self) -> Vec<u8> {
        let mut result = Vec::with_capacity(self.total_len());
        for part in &self.parts {
            result.extend_from_slice(part);
        }
        result
    }

    /// scatter/gather読み込み用イテレータ
    pub fn iter_parts(&self) -> impl Iterator<Item = &[u8]> {
        self.parts.iter().copied()
    }
}

// Note: AEAD integration functions have been removed due to unavailable dependencies.
// These would be implemented when proper AEAD support is available.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zero_copy::manager::{Buffer, BufferPool};

    #[test]
    fn byte_view_single() {
        let manager = BufferPool::with_capacity(1024);
        let buf_vec = manager.acquire(16);
        let buf = Buffer::from_vec(buf_vec);
        let view = ByteView::single(&buf);
        assert_eq!(view.total_len(), 16);
        assert_eq!(view.parts.len(), 1);
    }

    #[test]
    fn byte_view_multi() {
        let data1 = b"hello";
        let data2 = b"world";
        let view = ByteView::multi(vec![data1, data2]);
        assert_eq!(view.total_len(), 10);
        assert_eq!(view.to_vec(), b"helloworld");
    }

    #[test]
    fn into_bytes_conversion() {
        let manager = BufferPool::with_capacity(1024);
        let buf_vec = manager.acquire(8);
        let buf = Buffer::from_vec(buf_vec);
        let bytes = into_bytes(&buf);
        assert_eq!(bytes.len(), 8);
    }
}
