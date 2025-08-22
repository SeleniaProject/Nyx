#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

use crate::plugin::PluginId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
	Handshake,
	DataAccess,
	Control,
	ErrorReporting,
}

#[derive(Debug, Clone)]
pub struct PluginInfo {
	pub id: PluginId,
	pub name: String,
	pub permission_s: HashSet<Permission>,
}

impl PluginInfo {
	pub fn new(__id: PluginId, _name: impl Into<String>, permission_s: impl IntoIterator<Item = Permission>) -> Self {
		Self { id: __id, name: _name.into(), permission_s: permission_s.into_iter().collect() }
	}
}

#[derive(Debug)]
pub struct PluginRegistry {
	pub(crate) inner: Arc<RwLock<HashMap<PluginId, PluginInfo>>>,
}

impl PluginRegistry {
	pub fn new() -> Self { Self { inner: Arc::new(RwLock::new(HashMap::new())) } }

	pub async fn register(&self, info: PluginInfo) -> Result<(), &'static str> {
		let mut m = self.inner.write().await;
		if m.contains_key(&info.id) { return Err("already registered"); }
		m.insert(info.id, info);
		Ok(())
	}

	pub async fn unregister(&self, id: PluginId) -> Result<(), &'static str> {
		let mut m = self.inner.write().await;
		if m.remove(&id).is_none() { return Err("not registered"); }
		Ok(())
	}

	pub async fn is_registered(&self, id: PluginId) -> bool {
		let m = self.inner.read().await;
		m.contains_key(&id)
	}

	pub async fn has_permission(&self, id: PluginId, perm: Permission) -> bool {
		let m = self.inner.read().await;
		m.get(&id).map(|i| i.permission_s.contains(&perm)).unwrap_or(false)
	}

	pub async fn count(&self) -> usize {
		let m = self.inner.read().await;
		m.len()
	}
}

impl Default for PluginRegistry {
	fn default() -> Self { Self::new() }
}
