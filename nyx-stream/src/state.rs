#![forbid(unsafe_code)]

//! Stream state machine handling send/recv lifecycle and retransmission bookkeeping.
//! The implementation follows the design §4.3 with states: Idle → Open → HalfClosed → Closed.
//! For brevity we model HalfClosed in two variants depending on which side closed.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use std::collections::{HashMap, VecDeque};
use tokio::time::sleep;
use tokio::sync::mpsc;

/// Stream logical state.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum StreamState {
    Idle,
    Open,
    // Local ended writing, still expecting remote data.
    HalfClosedLocal,
    // Remote closed writing, local can still send.
    HalfClosedRemote,
    Closed,
}

/// Sent but unacked segment.
#[derive(Debug, Clone)]
struct SentSegment {
    offset: u32,
    len: usize,
    timestamp: Instant,
    /// Cached segment data for retransmission
    data: Vec<u8>,
}

/// Reassembly buffer entry.
#[derive(Debug)]
#[allow(dead_code)]
struct RecvSegment {
    offset: u32,
    data: Vec<u8>,
}

/// Core stream structure bound to a single Connection ID (CID).
#[allow(dead_code)]
pub struct Stream {
    pub id: u32,
    state: StreamState,
    send_offset: u32,
    recv_offset: u32,
    sent: BTreeMap<u32, SentSegment>, // keyed by offset
    recv_buffer: Vec<RecvSegment>,
    ack_tx: mpsc::Sender<u32>, // offset to acknowledge (largest)
    rto: Duration,
    /// Advanced fake data cache for pattern reuse and generation
    fake_data_cache: FakeDataCache,
}

impl Stream {
    /// Create new stream in Idle state.
    pub fn new(id: u32, ack_tx: mpsc::Sender<u32>) -> Self {
        Self {
            id,
            state: StreamState::Idle,
            send_offset: 0,
            recv_offset: 0,
            sent: BTreeMap::new(),
            recv_buffer: Vec::new(),
            ack_tx,
            rto: Duration::from_millis(250),
            fake_data_cache: FakeDataCache::new(1024), // 1KB cache size
        }
    }

    /// Transition Idle → Open on first send.
    pub fn send_data(&mut self, data: &[u8]) -> Vec<u8> {
        assert!(matches!(self.state, StreamState::Idle | StreamState::Open | StreamState::HalfClosedRemote));
        if self.state == StreamState::Idle {
            self.state = StreamState::Open;
        }
        let frame = crate::stream_frame::StreamFrame {
            stream_id: self.id,
            offset: self.send_offset,
            fin: false,
            data,
        };
        let bytes = crate::stream_frame::build_stream_frame(&frame);
        self.sent.insert(self.send_offset, SentSegment { 
            offset: self.send_offset, 
            len: data.len(), 
            timestamp: Instant::now(),
            data: data.to_vec(),
        });
        self.send_offset += data.len() as u32;
        bytes
    }

    /// Mark local side finished sending.
    pub fn finish(&mut self) -> Option<Vec<u8>> {
        if matches!(self.state, StreamState::Open | StreamState::HalfClosedRemote) {
            let frame = crate::stream_frame::StreamFrame {
                stream_id: self.id,
                offset: self.send_offset,
                fin: true,
                data: &[],
            };
            self.state = match self.state {
                StreamState::Open => StreamState::HalfClosedLocal,
                StreamState::HalfClosedRemote => StreamState::Closed,
                s => s,
            };
            Some(crate::stream_frame::build_stream_frame(&frame))
        } else { None }
    }

    /// Handle stream data from peer.
    pub fn on_receive(&mut self, frame: crate::stream_frame::StreamFrame<'_>) {
        // push to buffer; in-order delivery simplified.
        if frame.offset == self.recv_offset {
            self.recv_offset += frame.data.len() as u32;
        }
        // schedule ACK for largest offset seen
        let _ = self.ack_tx.try_send(self.recv_offset);

        if frame.fin {
            self.state = match self.state {
                StreamState::Open => StreamState::HalfClosedRemote,
                StreamState::HalfClosedLocal => StreamState::Closed,
                s => s,
            };
        }
    }

    /// Handle ACK frame acknowledging up to `largest`.
    pub fn on_ack(&mut self, largest: u32) {
        let mut to_remove = vec![];
        for (&off, _) in self.sent.iter() {
            if off + self.sent[&off].len as u32 <= largest {
                to_remove.push(off);
            }
        }
        for off in to_remove { self.sent.remove(&off); }
    }

    /// Periodic timer: detect losses (RTO) and request retransmission.
    pub async fn loss_retransmit_loop(mut self, tx: mpsc::Sender<Vec<u8>>) {
        loop {
            sleep(self.rto).await;
            let now = Instant::now();
            let mut lost: Vec<SentSegment> = Vec::new();
            for seg in self.sent.values() {
                if now.duration_since(seg.timestamp) >= self.rto {
                    lost.push(seg.clone());
                }
            }
            for seg in lost {
                if let Some(orig) = self.sent.get_mut(&seg.offset) {
                    orig.timestamp = Instant::now();
                }
                // Build retransmission frame with cached data
                let frame = crate::stream_frame::StreamFrame {
                    stream_id: self.id,
                    offset: seg.offset,
                    fin: false,
                    data: &seg.data,
                };
                let _ = tx.send(crate::stream_frame::build_stream_frame(&frame)).await;
            }
        }
    }

    pub fn state(&self) -> StreamState { self.state }

    /// Generate fake data using the advanced cache for testing and simulation
    pub fn generate_fake_data(&mut self, size: usize) -> Vec<u8> {
        self.fake_data_cache.generate_fake_data(size)
    }

    /// Get fake data cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        self.fake_data_cache.stats()
    }

    /// Clean up expired cache entries
    pub fn cleanup_cache(&mut self, max_age: Duration) {
        self.fake_data_cache.cleanup_expired(max_age);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitions() {
        let (ack_tx, _rx) = mpsc::channel(1);
        let mut s = Stream::new(1, ack_tx);
        assert_eq!(s.state(), StreamState::Idle);
        let _ = s.send_data(&[1,2]);
        assert_eq!(s.state(), StreamState::Open);
        let fin = s.finish();
        assert!(fin.is_some());
        assert_eq!(s.state(), StreamState::HalfClosedLocal);
    }
}

/// Advanced cache mechanism for fake data patterns
#[derive(Debug, Clone)]
pub struct FakeDataCache {
    /// Pattern storage with LRU eviction
    patterns: HashMap<u64, CacheEntry>,
    /// Access order for LRU
    access_order: VecDeque<u64>,
    /// Maximum cache size
    max_size: usize,
    /// Cache hit statistics
    hit_count: u64,
    /// Cache miss statistics  
    miss_count: u64,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    data: Vec<u8>,
    frequency: u32,
    last_access: Instant,
    creation_time: Instant,
}

impl FakeDataCache {
    /// Create new fake data cache with specified capacity
    pub fn new(max_size: usize) -> Self {
        Self {
            patterns: HashMap::new(),
            access_order: VecDeque::new(),
            max_size: max_size.max(1),
            hit_count: 0,
            miss_count: 0,
        }
    }

    /// Store fake data pattern with computed hash key
    pub fn store(&mut self, data: Vec<u8>) -> u64 {
        let hash_key = self.compute_hash(&data);
        
        if self.patterns.contains_key(&hash_key) {
            // Update existing entry
            if let Some(entry) = self.patterns.get_mut(&hash_key) {
                entry.frequency += 1;
                entry.last_access = Instant::now();
                self.update_lru_order(hash_key);
            }
        } else {
            // Insert new entry
            let entry = CacheEntry {
                data: data.clone(),
                frequency: 1,
                last_access: Instant::now(),
                creation_time: Instant::now(),
            };
            
            // Evict if necessary
            if self.patterns.len() >= self.max_size {
                self.evict_lru();
            }
            
            self.patterns.insert(hash_key, entry);
            self.access_order.push_back(hash_key);
        }
        
        hash_key
    }

    /// Retrieve fake data pattern by hash key
    pub fn retrieve(&mut self, hash_key: u64) -> Option<Vec<u8>> {
        let data = if let Some(entry) = self.patterns.get_mut(&hash_key) {
            entry.frequency += 1;
            entry.last_access = Instant::now();
            self.hit_count += 1;
            Some(entry.data.clone())
        } else {
            self.miss_count += 1;
            None
        };
        
        if data.is_some() {
            self.update_lru_order(hash_key);
        }
        
        data
    }

    /// Generate fake data using cached patterns or create new
    pub fn generate_fake_data(&mut self, size: usize) -> Vec<u8> {
        // Try to find similar-sized cached pattern
        let best_match = self.patterns
            .iter()
            .filter(|(_, entry)| entry.data.len() <= size * 2 && entry.data.len() >= size / 2)
            .max_by_key(|(_, entry)| entry.frequency)
            .map(|(key, _)| *key);

        if let Some(key) = best_match {
            if let Some(mut base_data) = self.retrieve(key) {
                // Resize to match requested size
                if base_data.len() < size {
                    // Extend with pattern repetition
                    while base_data.len() < size {
                        let remaining = size - base_data.len();
                        let to_copy = remaining.min(base_data.len());
                        let extension = base_data[0..to_copy].to_vec();
                        base_data.extend(extension);
                    }
                } else if base_data.len() > size {
                    base_data.truncate(size);
                }
                return base_data;
            }
        }

        // Generate new fake data if no suitable cached pattern
        let mut data = vec![0u8; size];
        let mut rng = fastrand::Rng::new();
        
        // Create pseudo-realistic pattern
        for i in 0..size {
            data[i] = match i % 4 {
                0 => rng.u8(0x20..0x7F), // ASCII printable
                1 => rng.u8(0x00..0x20), // Control chars
                2 => rng.u8(0x80..0xFF), // High bytes
                _ => (i % 256) as u8,     // Sequential pattern
            };
        }

        // Store generated pattern for future use
        self.store(data.clone());
        data
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            size: self.patterns.len(),
            max_size: self.max_size,
            hit_count: self.hit_count,
            miss_count: self.miss_count,
            hit_rate: if self.hit_count + self.miss_count > 0 {
                self.hit_count as f64 / (self.hit_count + self.miss_count) as f64
            } else {
                0.0
            },
        }
    }

    /// Clear expired entries based on age
    pub fn cleanup_expired(&mut self, max_age: Duration) {
        let now = Instant::now();
        let expired_keys: Vec<u64> = self.patterns
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.creation_time) > max_age)
            .map(|(key, _)| *key)
            .collect();

        for key in expired_keys {
            self.patterns.remove(&key);
            self.access_order.retain(|&k| k != key);
        }
    }

    /// Compute hash for data pattern
    fn compute_hash(&self, data: &[u8]) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish()
    }

    /// Update LRU access order
    fn update_lru_order(&mut self, key: u64) {
        // Remove from current position
        self.access_order.retain(|&k| k != key);
        // Add to end (most recently used)
        self.access_order.push_back(key);
    }

    /// Evict least recently used entry
    fn evict_lru(&mut self) {
        if let Some(lru_key) = self.access_order.pop_front() {
            self.patterns.remove(&lru_key);
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub hit_rate: f64,
}