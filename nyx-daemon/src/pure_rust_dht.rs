#![forbid(unsafe_code)]
//! Minimal in-process DHT with optional persistence. Supports put/get with TTL and
//! region/capability indexing; can persist KVS and indices to a redb database when enabled.

use base64::{engine::general_purpose, Engine as _};
use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::RwLock as AsyncRw;

#[derive(Clone, Debug)]
pub struct DhtValue {
    pub data: Vec<u8>,
    pub inserted: Instant,
    pub ttl: Duration,
    pub region: Option<String>,
    pub caps: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct InMemoryDht {
    inner: Arc<AsyncRw<HashMap<String, DhtValue>>>,
    region_index: Arc<AsyncRw<HashMap<String, HashSet<String>>>>,
    cap_index: Arc<AsyncRw<HashMap<String, HashSet<String>>>>,
    listen_addr: Arc<String>,
    db: Option<Arc<Database>>,
}

impl Default for InMemoryDht {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize)]
struct PersistentRecord {
    data_b64: String,
    expire_unix: u64,
    region: Option<String>,
    caps: Vec<String>,
}

impl InMemoryDht {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AsyncRw::new(HashMap::new())),
            region_index: Arc::new(AsyncRw::new(HashMap::new())),
            cap_index: Arc::new(AsyncRw::new(HashMap::new())),
            listen_addr: Arc::new("127.0.0.1:4330".to_string()),
            db: None,
        }
    }

    /// Create a DHT with persistence enabled at the given path. Loads existing entries.
    pub fn new_with_persistence<P: Into<String>>(path: P) -> Self {
        let mut dht = Self::new();
        let path_str = path.into();
        dht.db = Database::create(path_str).ok().map(Arc::new);
        // Best-effort load from store
        let _ = dht.load_from_store();
        dht
    }

    pub async fn put(
        &self,
        key: String,
        data: Vec<u8>,
        ttl: Duration,
        region: Option<String>,
        caps: &[String],
    ) {
        let caps_vec: Vec<String> = caps.to_vec();
        self.inner.write().await.insert(
            key.clone(),
            DhtValue {
                data,
                inserted: Instant::now(),
                ttl,
                region: region.clone(),
                caps: caps_vec.clone(),
            },
        );
        if let Some(ref r) = region {
            self.region_index
                .write()
                .await
                .entry(r.clone())
                .or_default()
                .insert(key.clone());
        }
        for c in &caps_vec {
            self.cap_index
                .write()
                .await
                .entry(c.clone())
                .or_default()
                .insert(key.clone());
        }
        let _ = self.save_one_to_store(&key).ok();
    }

    // Compatibility helper: simplified put without ttl/indices (default 5m TTL)
    pub async fn put_simple(&self, key: &str, value: Vec<u8>) {
        self.put(key.to_string(), value, Duration::from_secs(300), None, &[])
            .await;
    }

    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let mut guard = self.inner.write().await; // prune expired
        if let Some(v) = guard.get(key) {
            if v.inserted.elapsed() > v.ttl {
                guard.remove(key);
                return None;
            }
        }
        guard.get(key).map(|v| v.data.clone())
    }

    /// Enable persistence using environment variable `NYX_DHT_DB` or default path.
    pub fn enable_persistence_from_env(&mut self) {
        let path = std::env::var("NYX_DHT_DB").unwrap_or_else(|_| "nyx_dht.redb".to_string());
        self.db = Database::create(path).ok().map(Arc::new);
        let _ = self.load_from_store();
    }

    fn table_def() -> TableDefinition<'static, &'static str, &'static str> {
        TableDefinition::new("dht_kvs")
    }

    fn save_one_to_store(&self, key: &str) -> anyhow::Result<()> {
        let db = match &self.db {
            Some(db) => db.clone(),
            None => return Ok(()),
        };
        let guard = futures::executor::block_on(self.inner.read());
        if let Some(val) = guard.get(key) {
            // Compute expiry as unix seconds
            let now = SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let expire_unix = now.saturating_add(val.ttl.as_secs());
            let rec = PersistentRecord {
                data_b64: general_purpose::STANDARD.encode(&val.data),
                expire_unix,
                region: val.region.clone(),
                caps: val.caps.clone(),
            };
            let json = serde_json::to_string(&rec)?;
            let wtx = db.begin_write()?;
            // Open existing table; if missing, skip persistence silently
            if let Ok(mut table) = wtx.open_table(Self::table_def()) {
                table.insert(key, json.as_str())?;
            }
            wtx.commit()?;
        }
        Ok(())
    }

    fn load_from_store(&mut self) -> anyhow::Result<()> {
        let db = match &self.db {
            Some(db) => db.clone(),
            None => return Ok(()),
        };
        let rtx = db.begin_read()?;
        use redb::ReadableTable;
        let table = rtx.open_table(Self::table_def())?;
        for item in table.iter()? {
            let entry = item?;
            let key = entry.0.value();
            let json = entry.1.value();
            if let Ok(rec) = serde_json::from_str::<PersistentRecord>(json) {
                // Reconstruct TTL based on expire time
                let now_sec = SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if rec.expire_unix <= now_sec {
                    continue;
                }
                let ttl_secs = rec.expire_unix - now_sec;
                let data = match general_purpose::STANDARD.decode(rec.data_b64.as_bytes()) {
                    Ok(d) => d,
                    Err(_) => continue,
                };
                // Insert into maps
                futures::executor::block_on(async {
                    self.inner.write().await.insert(
                        key.to_string(),
                        DhtValue {
                            data,
                            inserted: Instant::now(),
                            ttl: Duration::from_secs(ttl_secs),
                            region: rec.region.clone(),
                            caps: rec.caps.clone(),
                        },
                    );
                    if let Some(r) = rec.region.clone() {
                        self.region_index
                            .write()
                            .await
                            .entry(r)
                            .or_default()
                            .insert(key.to_string());
                    }
                    for c in rec.caps.iter() {
                        self.cap_index
                            .write()
                            .await
                            .entry(c.clone())
                            .or_default()
                            .insert(key.to_string());
                    }
                });
            }
        }
        Ok(())
    }

    /// Start a periodic GC task to prune expired entries and clean indices.
    pub fn start_gc(&self, interval: Duration) {
        let inner = self.inner.clone();
        let region_index = self.region_index.clone();
        let cap_index = self.cap_index.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                // Phase 1: remove expired from inner
                {
                    let mut guard = inner.write().await;
                    let now = Instant::now();
                    let mut to_remove: Vec<String> = Vec::new();
                    for (k, v) in guard.iter() {
                        if now.duration_since(v.inserted) > v.ttl {
                            to_remove.push(k.clone());
                        }
                    }
                    for k in &to_remove {
                        guard.remove(k);
                    }
                }
                // Phase 2: scrub indices of non-existent keys
                let existing: std::collections::HashSet<String> = {
                    let g = inner.read().await;
                    g.keys().cloned().collect()
                };
                {
                    let mut ridx = region_index.write().await;
                    ridx.retain(|_, set| {
                        set.retain(|k| existing.contains(k));
                        !set.is_empty()
                    });
                }
                {
                    let mut cidx = cap_index.write().await;
                    cidx.retain(|_, set| {
                        set.retain(|k| existing.contains(k));
                        !set.is_empty()
                    });
                }
            }
        });
    }

    // Enumerate keys inside backing store matching a prefix (internal helper for discovery strategies)
    pub async fn keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        let guard = self.inner.read().await;
        guard
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }

    // Convenience: list region index listing keys ("region:<name>")
    pub async fn list_region_listing_keys(&self) -> Vec<String> {
        self.keys_with_prefix("region:").await
    }

    pub async fn by_region(&self, region: &str) -> Vec<Vec<u8>> {
        if let Some(keys) = self.region_index.read().await.get(region) {
            let mut out = Vec::new();
            for k in keys.iter() {
                if let Some(v) = self.get(k).await {
                    out.push(v);
                }
            }
            out
        } else {
            vec![]
        }
    }

    pub fn listen_addr(&self) -> &str {
        &self.listen_addr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;
    #[tokio::test]
    async fn put_get_roundtrip() {
        let d = InMemoryDht::new();
        d.put(
            "k".into(),
            b"v".to_vec(),
            Duration::from_millis(50),
            Some("JP".into()),
            &[],
        )
        .await;
        assert_eq!(d.get("k").await.unwrap(), b"v".to_vec());
    }
}

// cross-module integration smoke test (only compiled with path-builder feature)
#[cfg(all(test, feature = "path-builder"))]
mod pb_integration_tests {

    use crate::path_builder_broken::{DhtPeerDiscovery, DiscoveryCriteria, DummyDhtHandle};
    use std::sync::Arc;

    #[tokio::test]
    async fn region_discovery_from_inmemory_dht() {
        let handle = Arc::new(DummyDhtHandle::new());
        // Insert a peer record manually
        let peer_id = "peer_test_1".to_string();
        let region = "test_region".to_string();
        let peer_record = format!("{}|127.0.0.1:4330|10.0|100.0|active|0|{}", peer_id, region);
        handle
            .put(&format!("peer:{}", peer_id), peer_record.into_bytes())
            .await
            .unwrap();
        // region index
        let region_list = serde_json::to_vec(&vec![peer_id.clone()]).unwrap();
        handle
            .put(&format!("region:{}", region), region_list)
            .await
            .unwrap();
        let mut discovery = DhtPeerDiscovery::new(handle);
        let peers = discovery
            .discover_peers(DiscoveryCriteria::ByRegion("test_region".to_string()))
            .await
            .expect("region discovery");
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].region, "test_region");
    }

    #[tokio::test]
    async fn capability_and_random_all_discovery() {
        let handle = Arc::new(DummyDhtHandle::new());
        // two peers, same capability "relay"
        for (i, reg) in ["r1", "r2"].iter().enumerate() {
            let peer_id = format!("p{}", i);
            let rec = format!("{}|127.0.0.1:433{}|5.0|50.0|active|0|{}", peer_id, i, reg);
            handle
                .put(&format!("peer:{}", peer_id), rec.into_bytes())
                .await
                .unwrap();
        }
        // region lists
        let _ = handle
            .put(
                "region:r1",
                serde_json::to_vec(&vec!["p0".to_string()]).unwrap(),
            )
            .await;
        let _ = handle
            .put(
                "region:r2",
                serde_json::to_vec(&vec!["p1".to_string()]).unwrap(),
            )
            .await;
        // capability index (JSON list of peer IDs)
        let _ = handle
            .put(
                "cap:relay",
                serde_json::to_vec(&vec!["p0".to_string(), "p1".to_string()]).unwrap(),
            )
            .await;
        let mut discovery = DhtPeerDiscovery::new(handle);
        let cap = discovery
            .discover_peers(DiscoveryCriteria::ByCapability("relay".into()))
            .await
            .expect("cap");
        assert_eq!(cap.len(), 2);
        let any = discovery
            .discover_peers(DiscoveryCriteria::Random(1))
            .await
            .expect("random");
        assert!(any.len() >= 1 && any.len() <= 2); // cache 合併で >1 になる可能性許容
        let all = discovery
            .discover_peers(DiscoveryCriteria::All)
            .await
            .expect("all");
        assert!(all.len() >= 2);
    }
}
