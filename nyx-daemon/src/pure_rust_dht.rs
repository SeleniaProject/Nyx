#![forbid(unsafe_code)]
//! Minimal in-process DHT stub replacing placeholder; supports put/get with TTL and
//! region/capability indexing for path builder integration tests.

use std::{collections::{HashMap, HashSet}, time::{Instant, Duration}, sync::Arc};
use tokio::sync::RwLock as AsyncRw;

#[derive(Clone, Debug)]
pub struct DhtValue { pub data: Vec<u8>, pub inserted: Instant, pub ttl: Duration }

#[derive(Clone, Default, Debug)]
pub struct InMemoryDht {
	inner: Arc<AsyncRw<HashMap<String, DhtValue>>>,
	region_index: Arc<AsyncRw<HashMap<String, HashSet<String>>>>,
	cap_index: Arc<AsyncRw<HashMap<String, HashSet<String>>>>,
	listen_addr: Arc<String>,
}

impl InMemoryDht {
	pub fn new() -> Self { Self { listen_addr: Arc::new("127.0.0.1:4330".to_string()), ..Default::default() } }

	pub async fn put(&self, key: String, data: Vec<u8>, ttl: Duration, region: Option<String>, caps: &[String]) {
		self.inner.write().await.insert(key.clone(), DhtValue { data, inserted: Instant::now(), ttl });
		if let Some(r) = region { self.region_index.write().await.entry(r).or_default().insert(key.clone()); }
		for c in caps { self.cap_index.write().await.entry(c.clone()).or_default().insert(key.clone()); }
	}

	// Compatibility helper: simplified put without ttl/indices (default 5m TTL)
	pub async fn put_simple(&self, key: &str, value: Vec<u8>) { self.put(key.to_string(), value, Duration::from_secs(300), None, &[]).await; }

	pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
		let mut guard = self.inner.write().await; // prune expired
		if let Some(v) = guard.get(key) { if v.inserted.elapsed() > v.ttl { guard.remove(key); return None; } }
		guard.get(key).map(|v| v.data.clone())
	}

	// Enumerate keys inside backing store matching a prefix (internal helper for discovery strategies)
	pub async fn keys_with_prefix(&self, prefix: &str) -> Vec<String> {
		let guard = self.inner.read().await;
		guard.keys().filter(|k| k.starts_with(prefix)).cloned().collect()
	}

	// Convenience: list region index listing keys ("region:<name>")
	pub async fn list_region_listing_keys(&self) -> Vec<String> { self.keys_with_prefix("region:").await }

	pub async fn by_region(&self, region: &str) -> Vec<Vec<u8>> {
		if let Some(keys) = self.region_index.read().await.get(region) {
			let mut out = Vec::new();
			for k in keys.iter() { if let Some(v) = self.get(k).await { out.push(v); } }
			out
		} else { vec![] }
	}

	pub fn listen_addr(&self) -> &str { &self.listen_addr }
}

#[cfg(test)]
mod tests { use super::*; use tokio::time::Duration; #[tokio::test] async fn put_get_roundtrip() { let d=InMemoryDht::new(); d.put("k".into(), b"v".to_vec(), Duration::from_millis(50), Some("JP".into()), &[]).await; assert_eq!(d.get("k").await.unwrap(), b"v".to_vec()); } }
 
// cross-module integration smoke test (only compiled with path-builder feature)
#[cfg(all(test, feature="path-builder"))]
mod pb_integration_tests {
	use super::*;
	use crate::path_builder_broken::{DhtPeerDiscovery, DiscoveryCriteria, DummyDhtHandle};
	use std::sync::Arc;
	use tokio::time::Duration;
	#[tokio::test]
	async fn region_discovery_from_inmemory_dht() {
		let handle = Arc::new(DummyDhtHandle::new());
		// Insert a peer record manually
		let peer_id = "peer_test_1".to_string();
		let region = "test_region".to_string();
		let peer_record = format!("{}|127.0.0.1:4330|10.0|100.0|active|0|{}", peer_id, region);
		handle.put(&format!("peer:{}", peer_id), peer_record.into_bytes()).await.unwrap();
		// region index
		let region_list = serde_json::to_vec(&vec![peer_id.clone()]).unwrap();
		handle.put(&format!("region:{}", region), region_list).await.unwrap();
		let mut discovery = DhtPeerDiscovery::new(handle);
		let peers = discovery.discover_peers(DiscoveryCriteria::ByRegion("test_region".to_string())).await.expect("region discovery");
		assert_eq!(peers.len(), 1); assert_eq!(peers[0].region, "test_region");
	}

	#[tokio::test]
	async fn capability_and_random_all_discovery() {
		let handle = Arc::new(DummyDhtHandle::new());
		// two peers, same capability "relay"
		for (i, reg) in ["r1", "r2"].iter().enumerate() {
			let peer_id = format!("p{}", i);
			let rec = format!("{}|127.0.0.1:433{}|5.0|50.0|active|0|{}", peer_id, i, reg);
			handle.put(&format!("peer:{}", peer_id), rec.into_bytes()).await.unwrap();
		}
		// region lists
		let _ = handle.put("region:r1", serde_json::to_vec(&vec!["p0".to_string()]).unwrap()).await;
		let _ = handle.put("region:r2", serde_json::to_vec(&vec!["p1".to_string()]).unwrap()).await;
		// capability index (JSON list of peer IDs)
		let _ = handle.put("cap:relay", serde_json::to_vec(&vec!["p0".to_string(), "p1".to_string()]).unwrap()).await;
		let mut discovery = DhtPeerDiscovery::new(handle);
		let cap = discovery.discover_peers(DiscoveryCriteria::ByCapability("relay".into())).await.expect("cap");
		assert_eq!(cap.len(), 2);
		let any = discovery.discover_peers(DiscoveryCriteria::Random(1)).await.expect("random");
		assert!(any.len() >= 1 && any.len() <= 2); // cache 合併で >1 になる可能性許容
		let all = discovery.discover_peers(DiscoveryCriteria::All).await.expect("all");
		assert!(all.len() >= 2);
	}
}
