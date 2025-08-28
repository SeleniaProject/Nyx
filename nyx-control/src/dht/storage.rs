#![forbid(unsafe_code)]

use crate::dht::types::{StorageKey, StorageValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistEntry {
    pub key: Vec<u8>,
    pub val: Vec<u8>,
    pub stored_at_ms: u128,
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EntryMeta {
    pub stored_at_ms: u128,
    pub ttl_secs: u64,
}

#[derive(Debug, Clone)]
pub struct DhtStorage {
    map: HashMap<StorageKey, (StorageValue, EntryMeta, Instant)>,
    capacity: usize,
}

impl Default for DhtStorage {
    fn default() -> Self { Self { map: HashMap::new(), capacity: 4096 } }
}

impl DhtStorage {
    pub fn new() -> Self { Self::default() }
    pub fn with_capacity(cap: usize) -> Self { Self { capacity: cap, ..Default::default() } }

    /// 新API: TTL指定のPUT
    pub fn put_with_ttl(&mut self, k: StorageKey, v: StorageValue, ttl: Duration) -> Result<(), &'static str> {
        self.gc();
        if self.map.len() >= self.capacity { return Err("capacity exceeded"); }
        let meta = EntryMeta { stored_at_ms: now_ms(), ttl_secs: ttl.as_secs() };
        self.map.insert(k, (v, meta, Instant::now()));
        Ok(())
    }

    /// 互換API: デフォルトTTL(1h)
    pub fn put(&mut self, k: StorageKey, v: StorageValue) -> Result<(), &'static str> {
        self.put_with_ttl(k, v, Duration::from_secs(3600))
    }

    pub fn get(&mut self, k: &StorageKey) -> Option<StorageValue> {
        self.gc();
        self.map.get(k).map(|(v, _, _)| v.clone())
    }

    pub fn meta(&mut self, k: &StorageKey) -> Option<EntryMeta> {
        self.gc();
        self.map.get(k).map(|(_, m, _)| m.clone())
    }

    pub fn delete(&mut self, k: &StorageKey) -> Result<bool, &'static str> { Ok(self.map.remove(k).is_some()) }
    pub fn capacity(&self) -> usize { self.capacity }
    pub fn len(&self) -> usize { self.map.len() }
    pub fn is_empty(&self) -> bool { self.map.is_empty() }

    pub fn gc(&mut self) {
        let now = Instant::now();
        self.map.retain(|_, (_, m, ins)| now.duration_since(*ins) < Duration::from_secs(m.ttl_secs));
    }

    // ---- Persistence helpers ----
    pub fn export_persist(&mut self) -> Vec<PersistEntry> {
        self.gc();
        self.map
            .iter()
            .map(|(k, (v, m, _))| PersistEntry { key: k.0.clone(), val: v.0.clone(), stored_at_ms: m.stored_at_ms, ttl_secs: m.ttl_secs })
            .collect()
    }

    pub fn import_persist(&mut self, entries: Vec<PersistEntry>) {
        self.map.clear();
        let now_ms_v = now_ms();
        for e in entries {
            // 記録時刻からの経過で残TTLを計算
            let elapsed_ms = now_ms_v.saturating_sub(e.stored_at_ms) as u64;
            let elapsed_secs = elapsed_ms / 1000;
            if e.ttl_secs <= elapsed_secs { continue; }
            let remaining = e.ttl_secs - elapsed_secs;
            let _ = self.put_with_ttl(StorageKey(e.key), StorageValue(e.val), Duration::from_secs(remaining));
        }
    }
}

fn now_ms() -> u128 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() }
