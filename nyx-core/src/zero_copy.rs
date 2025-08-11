#![forbid(unsafe_code)]

//! Zero-copy optimization system for critical data path (crypto→FEC→transmission).
//!
//! This module provides comprehensive memory allocation tracking and reduction strategies
//! for the Nyx protocol's performance-critical data processing pipeline. The system 
//! monitors and optimizes memory usage across three key stages:
//!
//! 1. **Crypto Stage**: AEAD encryption/decryption buffer management
//! 2. **FEC Stage**: RaptorQ encoding/decoding memory optimization  
//! 3. **Transmission Stage**: Network buffer zero-copy strategies
//!
//! ## Key Features
//! 
//! - **Allocation Tracking**: Comprehensive counters for memory operations
//! - **Buffer Reuse**: Intelligent buffer pooling to minimize allocations
//! - **Zero-Copy Pipelines**: Direct memory mapping where possible
//! - **Performance Metrics**: Detailed telemetry integration
//! - **Adaptive Optimization**: Dynamic tuning based on usage patterns
//!
//! ## Usage
//! 
//! ```rust
//! use nyx_core::zero_copy::{ZeroCopyManager, CriticalPath};
//! 
//! let mut manager = ZeroCopyManager::new();
//! let path = manager.create_critical_path();
//! 
//! // Process data through zero-copy optimized pipeline
//! let result = path.process_packet(data).await?;
//! ```

use std::sync::{Arc, Mutex, atomic::{AtomicU64, AtomicUsize, Ordering}};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

pub mod manager;
pub mod telemetry;
pub mod integration;

/// Critical path stage identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    /// AEAD encryption/decryption stage
    Crypto,
    /// RaptorQ FEC encoding/decoding stage  
    Fec,
    /// Network transmission buffer stage
    Transmission,
}

/// Memory operation type for tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    /// New allocation
    Allocate,
    /// Buffer copy operation
    Copy,
    /// Buffer reallocation (resize)
    Reallocate,
    /// Buffer pool acquisition
    PoolGet,
    /// Buffer pool return
    PoolReturn,
    /// Zero-copy reference
    ZeroCopy,
}

/// Single allocation event for detailed tracking
#[derive(Debug, Clone)]
pub struct AllocationEvent {
    /// Stage where allocation occurred
    pub stage: Stage,
    /// Type of memory operation
    pub operation: OperationType,
    /// Size in bytes
    pub size: usize,
    /// Timestamp of operation
    pub timestamp: Instant,
    /// Optional context identifier
    pub context: Option<String>,
}

/// Aggregated allocation statistics per stage
#[derive(Debug, Clone)]
pub struct StageStats {
    /// Total allocations
    pub total_allocations: u64,
    /// Total bytes allocated
    pub total_bytes: u64,
    /// Total copy operations
    pub total_copies: u64,
    /// Total bytes copied
    pub total_copy_bytes: u64,
    /// Total reallocation events
    pub total_reallocations: u64,
    /// Pool hits (successful reuse)
    pub pool_hits: u64,
    /// Pool misses (new allocation needed)
    pub pool_misses: u64,
    /// Zero-copy operations
    pub zero_copy_ops: u64,
    /// Peak memory usage in bytes
    pub peak_memory: u64,
    /// Current active allocations
    pub active_allocations: u64,
    /// Average allocation size
    pub average_alloc_size: f64,
    /// Last update timestamp
    pub last_updated: Instant,
}

impl Default for StageStats {
    fn default() -> Self {
        Self {
            total_allocations: 0,
            total_bytes: 0,
            total_copies: 0,
            total_copy_bytes: 0,
            total_reallocations: 0,
            pool_hits: 0,
            pool_misses: 0,
            zero_copy_ops: 0,
            peak_memory: 0,
            active_allocations: 0,
            average_alloc_size: 0.0,
            last_updated: Instant::now(),
        }
    }
}

/// Comprehensive allocation tracking across critical path
#[derive(Debug, Default, Clone)]
pub struct AllocationMetrics {
    /// Per-stage statistics
    pub stages: HashMap<Stage, StageStats>,
    /// End-to-end pipeline stats
    pub pipeline_total_allocations: u64,
    pub pipeline_total_bytes: u64,
    pub pipeline_peak_memory: u64,
    /// Optimization effectiveness
    pub reduction_ratio: f64,
    pub zero_copy_ratio: f64,
    /// Performance impact
    pub allocation_overhead_ns: u64,
    pub copy_overhead_ns: u64,
}

/// Buffer pool for zero-copy optimization
#[derive(Debug, Clone)]
pub struct ZeroCopyBuffer {
    /// Raw buffer data
    pub data: Vec<u8>,
    /// Current capacity
    pub capacity: usize,
    /// Reference count for sharing
    pub ref_count: Arc<AtomicUsize>,
    /// Creation timestamp for age tracking
    pub created_at: Instant,
    /// Last access timestamp
    pub last_accessed: Instant,
}

impl ZeroCopyBuffer {
    /// Create new buffer with specified capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            capacity,
            ref_count: Arc::new(AtomicUsize::new(1)),
            created_at: Instant::now(),
            last_accessed: Instant::now(),
        }
    }

    /// Get mutable reference to buffer data
    pub fn as_mut(&mut self) -> &mut Vec<u8> {
        self.last_accessed = Instant::now();
        &mut self.data
    }

    /// Get immutable reference to buffer data
    pub fn as_ref(&self) -> &[u8] {
        &self.data
    }

    /// Check if buffer can be reused (single reference)
    pub fn can_reuse(&self) -> bool {
        self.ref_count.load(Ordering::Acquire) == 1
    }

    /// Clone buffer reference (increment ref count)
    pub fn clone_ref(&self) -> Arc<AtomicUsize> {
        self.ref_count.fetch_add(1, Ordering::AcqRel);
        Arc::clone(&self.ref_count)
    }
}

/// Buffer pool management for zero-copy operations
pub struct BufferPool {
    /// Available buffers by size class
    pools: HashMap<usize, VecDeque<ZeroCopyBuffer>>,
    /// Pool configuration
    max_buffers_per_size: usize,
    max_total_buffers: usize,
    /// Current statistics
    total_buffers: usize,
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
    /// Size classes for efficient pooling
    size_classes: Vec<usize>,
}

impl BufferPool {
    /// Create new buffer pool with configuration
    pub fn new(max_buffers_per_size: usize, max_total_buffers: usize) -> Self {
        let size_classes = vec![
            256,    // Small packets
            1280,   // RaptorQ symbol size
            4096,   // Standard page size
            8192,   // Large packets  
            16384,  // Very large packets
            32768,  // Maximum frame size
        ];

        Self {
            pools: HashMap::new(),
            max_buffers_per_size,
            max_total_buffers,
            total_buffers: 0,
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
            size_classes,
        }
    }

    /// Get buffer of appropriate size
    pub fn get_buffer(&mut self, size: usize) -> ZeroCopyBuffer {
        let size_class = self.find_size_class(size);
        
        if let Some(pool) = self.pools.get_mut(&size_class) {
            if let Some(mut buffer) = pool.pop_front() {
                if buffer.can_reuse() {
                    buffer.data.clear();
                    buffer.data.reserve(size);
                    buffer.last_accessed = Instant::now();
                    self.hits.fetch_add(1, Ordering::Relaxed);
                    return buffer;
                }
            }
        }

        // Pool miss - create new buffer
        self.misses.fetch_add(1, Ordering::Relaxed);
        ZeroCopyBuffer::new(size_class)
    }

    /// Return buffer to pool for reuse
    pub fn return_buffer(&mut self, buffer: ZeroCopyBuffer) {
        if !buffer.can_reuse() {
            return; // Still has references
        }

        let size_class = buffer.capacity;
        let pool = self.pools.entry(size_class).or_insert_with(VecDeque::new);

        if pool.len() < self.max_buffers_per_size && self.total_buffers < self.max_total_buffers {
            pool.push_back(buffer);
            self.total_buffers += 1;
        }
    }

    /// Find appropriate size class for requested size
    fn find_size_class(&self, size: usize) -> usize {
        for &class_size in &self.size_classes {
            if size <= class_size {
                return class_size;
            }
        }
        // For very large sizes, round up to next power of 2
        size.next_power_of_two()
    }

    /// Get pool statistics
    pub fn stats(&self) -> BufferPoolStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total_requests = hits + misses;
        
        BufferPoolStats {
            total_buffers: self.total_buffers,
            hits,
            misses,
            hit_ratio: if total_requests > 0 {
                hits as f64 / total_requests as f64
            } else {
                0.0
            },
            size_class_distribution: self.pools.iter()
                .map(|(&size, pool)| (size, pool.len()))
                .collect(),
        }
    }

    /// Cleanup old unused buffers
    pub fn cleanup(&mut self, max_age: Duration) {
        let now = Instant::now();
        let mut removed = 0;

        for pool in self.pools.values_mut() {
            pool.retain(|buffer| {
                let keep = now.duration_since(buffer.last_accessed) < max_age;
                if !keep {
                    removed += 1;
                }
                keep
            });
        }

        self.total_buffers -= removed;
        if removed > 0 {
            debug!("Cleaned up {} old buffers from pool", removed);
        }
    }
}

/// Buffer pool statistics
#[derive(Debug, Clone)]
pub struct BufferPoolStats {
    pub total_buffers: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_ratio: f64,
    pub size_class_distribution: HashMap<usize, usize>,
}

/// Zero-copy allocation tracker
pub struct AllocationTracker {
    /// Event history for detailed analysis
    events: Arc<RwLock<VecDeque<AllocationEvent>>>,
    /// Atomic counters for fast updates
    crypto_allocations: Arc<AtomicU64>,
    crypto_bytes: Arc<AtomicU64>,
    fec_allocations: Arc<AtomicU64>,
    fec_bytes: Arc<AtomicU64>,
    transmission_allocations: Arc<AtomicU64>,
    transmission_bytes: Arc<AtomicU64>,
    /// Peak memory tracking
    peak_memory: Arc<AtomicU64>,
    current_memory: Arc<AtomicU64>,
    /// Performance timing
    allocation_start_time: Arc<Mutex<Option<Instant>>>,
    total_allocation_time: Arc<AtomicU64>,
    /// Copy operation timing (separate from allocation timing)
    copy_start_time: Arc<Mutex<Option<Instant>>>,
    total_copy_time: Arc<AtomicU64>,
    /// Configuration
    max_events: usize,
}

impl AllocationTracker {
    /// Create new allocation tracker
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Arc::new(RwLock::new(VecDeque::with_capacity(max_events))),
            crypto_allocations: Arc::new(AtomicU64::new(0)),
            crypto_bytes: Arc::new(AtomicU64::new(0)),
            fec_allocations: Arc::new(AtomicU64::new(0)),
            fec_bytes: Arc::new(AtomicU64::new(0)),
            transmission_allocations: Arc::new(AtomicU64::new(0)),
            transmission_bytes: Arc::new(AtomicU64::new(0)),
            peak_memory: Arc::new(AtomicU64::new(0)),
            current_memory: Arc::new(AtomicU64::new(0)),
            allocation_start_time: Arc::new(Mutex::new(None)),
            total_allocation_time: Arc::new(AtomicU64::new(0)),
            copy_start_time: Arc::new(Mutex::new(None)),
            total_copy_time: Arc::new(AtomicU64::new(0)),
            max_events,
        }
    }

    /// Record allocation event
    pub async fn record_allocation(&self, event: AllocationEvent) {
        // Update atomic counters
        match event.stage {
            Stage::Crypto => {
                self.crypto_allocations.fetch_add(1, Ordering::Relaxed);
                self.crypto_bytes.fetch_add(event.size as u64, Ordering::Relaxed);
            }
            Stage::Fec => {
                self.fec_allocations.fetch_add(1, Ordering::Relaxed);
                self.fec_bytes.fetch_add(event.size as u64, Ordering::Relaxed);
            }
            Stage::Transmission => {
                self.transmission_allocations.fetch_add(1, Ordering::Relaxed);
                self.transmission_bytes.fetch_add(event.size as u64, Ordering::Relaxed);
            }
        }

        // Update current memory usage
        match event.operation {
            OperationType::Allocate | OperationType::Reallocate => {
                let current = self.current_memory.fetch_add(event.size as u64, Ordering::Relaxed) + event.size as u64;
                // Update peak if necessary
                self.peak_memory.fetch_max(current, Ordering::Relaxed);
            }
            OperationType::PoolReturn => {
                self.current_memory.fetch_sub(event.size as u64, Ordering::Relaxed);
            }
            _ => {}
        }

        // Store detailed event (with size limit)
        let mut events = self.events.write().await;
        if events.len() >= self.max_events {
            events.pop_front();
        }
        events.push_back(event);
    }

    /// Start timing allocation operation
    pub fn start_timing(&self) {
        *self.allocation_start_time.lock().unwrap() = Some(Instant::now());
    }

    /// End timing allocation operation
    pub fn end_timing(&self) {
        if let Some(start) = self.allocation_start_time.lock().unwrap().take() {
            let duration = start.elapsed().as_nanos() as u64;
            self.total_allocation_time.fetch_add(duration, Ordering::Relaxed);
        }
    }

    /// Start timing copy operation
    pub fn start_copy_timing(&self) {
        *self.copy_start_time.lock().unwrap() = Some(Instant::now());
    }

    /// End timing copy operation
    pub fn end_copy_timing(&self) {
        if let Some(start) = self.copy_start_time.lock().unwrap().take() {
            let duration = start.elapsed().as_nanos() as u64;
            self.total_copy_time.fetch_add(duration, Ordering::Relaxed);
        }
    }

    /// Get current metrics snapshot
    pub async fn get_metrics(&self) -> AllocationMetrics {
        let events = self.events.read().await;
        
        // Calculate per-stage statistics
        let mut stages = HashMap::new();
        
        for stage in [Stage::Crypto, Stage::Fec, Stage::Transmission] {
            let stage_events: Vec<_> = events.iter().filter(|e| e.stage == stage).collect();
            
            let total_allocations = stage_events.iter()
                .filter(|e| matches!(e.operation, OperationType::Allocate | OperationType::Reallocate))
                .count() as u64;
            
            let total_bytes: u64 = stage_events.iter()
                .filter(|e| matches!(e.operation, OperationType::Allocate | OperationType::Reallocate))
                .map(|e| e.size as u64)
                .sum();
            
            let total_copies = stage_events.iter()
                .filter(|e| e.operation == OperationType::Copy)
                .count() as u64;
            
            let total_copy_bytes: u64 = stage_events.iter()
                .filter(|e| e.operation == OperationType::Copy)
                .map(|e| e.size as u64)
                .sum();

            let zero_copy_ops = stage_events.iter()
                .filter(|e| e.operation == OperationType::ZeroCopy)
                .count() as u64;

            let pool_hits = stage_events.iter()
                .filter(|e| e.operation == OperationType::PoolGet)
                .count() as u64;

            let average_alloc_size = if total_allocations > 0 {
                total_bytes as f64 / total_allocations as f64
            } else {
                0.0
            };

            stages.insert(stage, StageStats {
                total_allocations,
                total_bytes,
                total_copies,
                total_copy_bytes,
                total_reallocations: stage_events.iter()
                    .filter(|e| e.operation == OperationType::Reallocate)
                    .count() as u64,
                pool_hits,
                pool_misses: total_allocations.saturating_sub(pool_hits),
                zero_copy_ops,
                peak_memory: 0, // Will be updated below
                active_allocations: 0, // Will be calculated from current state
                average_alloc_size,
                last_updated: Instant::now(),
            });
        }

        // Calculate pipeline totals
        let pipeline_total_allocations = stages.values().map(|s| s.total_allocations).sum();
        let pipeline_total_bytes = stages.values().map(|s| s.total_bytes).sum();
        let pipeline_peak_memory = self.peak_memory.load(Ordering::Relaxed);

        // Calculate optimization ratios
        let total_ops = pipeline_total_allocations + stages.values().map(|s| s.zero_copy_ops).sum::<u64>();
        let zero_copy_ratio = if total_ops > 0 {
            stages.values().map(|s| s.zero_copy_ops).sum::<u64>() as f64 / total_ops as f64
        } else {
            0.0
        };

        let reduction_ratio = if pipeline_total_bytes > 0 {
            1.0 - (stages.values().map(|s| s.total_copy_bytes).sum::<u64>() as f64 / pipeline_total_bytes as f64)
        } else {
            0.0
        };

        AllocationMetrics {
            stages,
            pipeline_total_allocations,
            pipeline_total_bytes,
            pipeline_peak_memory,
            reduction_ratio,
            zero_copy_ratio,
            allocation_overhead_ns: self.total_allocation_time.load(Ordering::Relaxed),
            copy_overhead_ns: self.total_copy_time.load(Ordering::Relaxed),
        }
    }

    /// Clear all tracking data
    pub async fn clear(&self) {
        self.events.write().await.clear();
        self.crypto_allocations.store(0, Ordering::Relaxed);
        self.crypto_bytes.store(0, Ordering::Relaxed);
        self.fec_allocations.store(0, Ordering::Relaxed);
        self.fec_bytes.store(0, Ordering::Relaxed);
        self.transmission_allocations.store(0, Ordering::Relaxed);
        self.transmission_bytes.store(0, Ordering::Relaxed);
        self.peak_memory.store(0, Ordering::Relaxed);
        self.current_memory.store(0, Ordering::Relaxed);
        self.total_allocation_time.store(0, Ordering::Relaxed);
        self.total_copy_time.store(0, Ordering::Relaxed);
    }
}
