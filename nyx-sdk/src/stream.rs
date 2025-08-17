#![forbid(unsafe_code)]

use bytes::Bytes;
use crate::error::{Error, Result};
use nyx_stream::async_stream::{AsyncStream, AsyncStreamConfig};

/// SDK 公開用のストリーム。内部は nyx-stream の AsyncStream に委譲する薄いアダプタ。
#[derive(Clone)]
pub struct NyxStream {
	inner: AsyncStream,
}

impl NyxStream {
	/// 開発/テスト向け：プロセス内で相互接続されたストリームペアを生成。
	pub fn pair(_stream_id: u32) -> (Self, Self) {
		let (a, b) = nyx_stream::async_stream::pair(AsyncStreamConfig::default(), AsyncStreamConfig::default());
		(Self { inner: a }, Self { inner: b })
	}

	pub async fn send(&self, data: impl Into<Bytes>) -> Result<()> {
		self.inner.send(data.into()).await.map_err(|e| Error::Protocol(e.to_string()))
	}

	/// 受信（ミリ秒タイムアウト）。期限までにデータがなければ Timeout。
	pub async fn recv(&self, timeout_ms: u64) -> Result<Option<Bytes>> {
		// 即時（ノンブロッキング）チェック
		if let Some(b) = self.inner.try_recv().await.map_err(|e| Error::Protocol(e.to_string()))? { return Ok(Some(b)); }
		if timeout_ms == 0 { return Ok(None); }
		let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
		while tokio::time::Instant::now() < deadline {
			if let Some(b) = self.inner.try_recv().await.map_err(|e| Error::Protocol(e.to_string()))? {
				return Ok(Some(b));
			}
			tokio::time::sleep(std::time::Duration::from_millis(1)).await;
		}
		Ok(None)
	}

	pub async fn close(&self) -> Result<()> {
		self.inner.close().await.map_err(|e| Error::Protocol(e.to_string()))
	}
}

