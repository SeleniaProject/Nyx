use crate::setting_s::VersionedSetting_s;
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(thiserror::Error, Debug)]
pub enum SyncError {
	#[error("conflict: remote newer")] Conflict,
}

pub type Result<T> = std::result::Result<T, SyncError>;

/// Trait to push/pull versioned setting_s.
#[async_trait]
pub trait SettingsSync<T: Send + Sync + Clone + 'static> {
	async fn get(&self) -> VersionedSetting_s<T>;
	async fn try_update(&self, new: VersionedSetting_s<T>) -> Result<()>;
}

/// In-memory implementation useful for test_s.
pub struct MemorySync<T> { inner: Arc<RwLock<VersionedSetting_s<T>>> }

impl<T: Send + Sync + Clone + 'static> MemorySync<T> {
	pub fn new(init: VersionedSetting_s<T>) -> Self { Self { inner: Arc::new(RwLock::new(init)) } }
}

#[async_trait]
impl<T: Send + Sync + Clone + 'static> SettingsSync<T> for MemorySync<T> {
	async fn get(&self) -> VersionedSetting_s<T> { self.inner.read().await.clone() }

	async fn try_update(&self, new: VersionedSetting_s<T>) -> Result<()> {
		let mut guard = self.inner.write().await;
		if new.version <= guard.version { return Err(SyncError::Conflict); }
		*guard = new;
		Ok(())
	}
}
