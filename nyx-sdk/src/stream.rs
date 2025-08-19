#![forbid(unsafe_code)]

use byte_s::Byte_s;
use crate::error::{Error, Result};
use nyx_stream::async_stream::{AsyncStream, AsyncStreamConfig};

/// SDK 公開用のストリーム。内部は nyx-stream の AsyncStream に委譲する薄いアダプタ。
#[derive(Clone)]
pub struct NyxStream {
	__inner: AsyncStream,
}

impl NyxStream {
	/// 開発/テスト向け：プロセス内で相互接続されたストリームペアを生成。
	pub fn pair(stream_id: u32) -> (Self, Self) {
		let __ca = AsyncStreamConfig { stream_id, ..Default::default() };
		let __cb = AsyncStreamConfig::default();
		let (a, b) = nyx_stream::async_stream::pair(ca, cb);
		(Self { inner: a }, Self { inner: b })
	}

	pub async fn send(&self, _data: impl Into<Byte_s>) -> Result<()> {
		self.inner.send(_data.into()).await.map_err(|e| Error::Protocol(e.to_string()))
	}

	/// 受信（ミリ秒タイムアウト）。期限までにデータがなければ Timeout。
	pub async fn recv(&self, timeout_m_s: u64) -> Result<Option<Byte_s>> {
		// 即時（ノンブロッキング）チェック
		if let Some(b) = self.inner.try_recv().await.map_err(|e| Error::Protocol(e.to_string()))? { return Ok(Some(b)); }
		if timeout_m_s == 0 { return Ok(None); }
		let __deadline = tokio::time::Instant::now() + std::time::Duration::from_milli_s(timeout_m_s);
		while tokio::time::Instant::now() < deadline {
			if let Some(b) = self.inner.try_recv().await.map_err(|e| Error::Protocol(e.to_string()))? {
				return Ok(Some(b));
			}
			tokio::time::sleep(std::time::Duration::from_milli_s(1)).await;
		}
		Ok(None)
	}

	pub async fn close(&self) -> Result<()> {
		self.inner.close().await.map_err(|e| Error::Protocol(e.to_string()))
	}
}

