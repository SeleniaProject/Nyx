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

/// AEAD暗号化用のヘルパー - scatter/gatherバッファを単一の暗号文にシール
#[cfg(feature = "aead")]
pub fn seal_scatter_gather(
    cipher: &crate::aead::AeadCipher,
    nonce: crate::aead::AeadNonce,
    aad: &[u8],
    view: &ByteView,
) -> crate::Result<Vec<u8>> {
    // 単一部分の場合は直接処理
    if view.parts.len() == 1 {
        return cipher.seal(nonce, aad, view.parts[0]);
    }

    // scatter/gatherの場合は一時的にコピー（将来的にはストリーミング実装）
    let plaintext = view.to_vec();
    cipher.seal(nonce, aad, &plaintext)
}

/// AEAD復号化用のヘルパー
#[cfg(feature = "aead")]
pub fn open_to_scatter(
    cipher: &crate::aead::AeadCipher,
    nonce: crate::aead::AeadNonce,
    aad: &[u8],
    ciphertext: &[u8],
) -> crate::Result<Vec<u8>> {
    // 現在は単純実装、将来的には指定されたバッファに直接復号
    cipher.open(nonce, aad, ciphertext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zero_copy::manager::ZeroCopyManager;

    #[test]
    fn byte_view_single() {
        let mut manager = ZeroCopyManager::new();
        let buf = manager.allocate(16).unwrap();
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
        let mut manager = ZeroCopyManager::new();
        let buf = manager.allocate(8).unwrap();
        let bytes = into_bytes(&buf);
        assert_eq!(bytes.len(), 8);
    }
}
