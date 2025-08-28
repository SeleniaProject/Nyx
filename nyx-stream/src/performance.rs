//! Ultra-high performance stream optimization module
//!
//! This module provides advanced performance optimizations for stream operations:
//! - Cache-aligned data structures for optimal memory access
//! - Lock-free atomic counters for metrics
//! - SIMD-optimized buffer operations where available
//! - Advanced memory pool management

use bytes::BytesMut;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Performance metrics collection for stream operations
#[repr(align(64))] // Cache line alignment
pub struct StreamMetrics {
    // Send metrics
    pub frames_sent: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub send_duration_nanos: AtomicU64,

    // Receive metrics
    pub frames_received: AtomicU64,
    pub bytes_received: AtomicU64,
    pub recv_duration_nanos: AtomicU64,

    // Buffer metrics
    pub buffer_allocations: AtomicU64,
    pub buffer_reallocations: AtomicU64,
    pub buffer_pool_hits: AtomicU64,
    pub buffer_pool_misses: AtomicU64,

    // Flow control metrics
    pub retransmissions: AtomicU64,
    pub congestion_events: AtomicU64,
    pub flow_control_blocks: AtomicU64,

    // Memory metrics
    pub peak_memory_usage: AtomicUsize,
    pub current_memory_usage: AtomicUsize,
}

impl Default for StreamMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamMetrics {
    pub fn new() -> Self {
        Self {
            frames_sent: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            send_duration_nanos: AtomicU64::new(0),
            frames_received: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            recv_duration_nanos: AtomicU64::new(0),
            buffer_allocations: AtomicU64::new(0),
            buffer_reallocations: AtomicU64::new(0),
            buffer_pool_hits: AtomicU64::new(0),
            buffer_pool_misses: AtomicU64::new(0),
            retransmissions: AtomicU64::new(0),
            congestion_events: AtomicU64::new(0),
            flow_control_blocks: AtomicU64::new(0),
            peak_memory_usage: AtomicUsize::new(0),
            current_memory_usage: AtomicUsize::new(0),
        }
    }

    #[inline]
    pub fn record_send(&self, bytes: usize, duration: Duration) {
        self.frames_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes as u64, Ordering::Relaxed);
        self.send_duration_nanos
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_recv(&self, bytes: usize, duration: Duration) {
        self.frames_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received
            .fetch_add(bytes as u64, Ordering::Relaxed);
        self.recv_duration_nanos
            .fetch_add(duration.as_nanos() as u64, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_buffer_allocation(&self, size: usize) {
        self.buffer_allocations.fetch_add(1, Ordering::Relaxed);
        self.update_memory_usage(size as isize);
    }

    #[inline]
    pub fn record_buffer_reallocation(&self) {
        self.buffer_reallocations.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_buffer_pool_hit(&self) {
        self.buffer_pool_hits.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_buffer_pool_miss(&self) {
        self.buffer_pool_misses.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_retransmission(&self) {
        self.retransmissions.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_congestion_event(&self) {
        self.congestion_events.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_flow_control_block(&self) {
        self.flow_control_blocks.fetch_add(1, Ordering::Relaxed);
    }

    fn update_memory_usage(&self, delta: isize) {
        let current = if delta >= 0 {
            self.current_memory_usage
                .fetch_add(delta as usize, Ordering::Relaxed)
                + delta as usize
        } else {
            self.current_memory_usage
                .fetch_sub((-delta) as usize, Ordering::Relaxed)
                - (-delta) as usize
        };

        // Update peak if necessary
        let mut peak = self.peak_memory_usage.load(Ordering::Relaxed);
        while current > peak {
            match self.peak_memory_usage.compare_exchange_weak(
                peak,
                current,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(new_peak) => peak = new_peak,
            }
        }
    }

    pub fn get_throughput_stats(&self) -> ThroughputStats {
        let frames_sent = self.frames_sent.load(Ordering::Relaxed);
        let bytes_sent = self.bytes_sent.load(Ordering::Relaxed);
        let send_duration = Duration::from_nanos(self.send_duration_nanos.load(Ordering::Relaxed));

        let frames_received = self.frames_received.load(Ordering::Relaxed);
        let bytes_received = self.bytes_received.load(Ordering::Relaxed);
        let recv_duration = Duration::from_nanos(self.recv_duration_nanos.load(Ordering::Relaxed));

        ThroughputStats {
            send_throughput_mbps: if send_duration.as_secs_f64() > 0.0 {
                (bytes_sent as f64 * 8.0) / (send_duration.as_secs_f64() * 1_000_000.0)
            } else {
                0.0
            },
            recv_throughput_mbps: if recv_duration.as_secs_f64() > 0.0 {
                (bytes_received as f64 * 8.0) / (recv_duration.as_secs_f64() * 1_000_000.0)
            } else {
                0.0
            },
            avg_send_latency_us: if frames_sent > 0 {
                (send_duration.as_micros() as f64) / (frames_sent as f64)
            } else {
                0.0
            },
            avg_recv_latency_us: if frames_received > 0 {
                (recv_duration.as_micros() as f64) / (frames_received as f64)
            } else {
                0.0
            },
            buffer_efficiency: {
                let hits = self.buffer_pool_hits.load(Ordering::Relaxed);
                let misses = self.buffer_pool_misses.load(Ordering::Relaxed);
                if hits + misses > 0 {
                    (hits as f64) / ((hits + misses) as f64)
                } else {
                    0.0
                }
            },
            retransmission_rate: if frames_sent > 0 {
                (self.retransmissions.load(Ordering::Relaxed) as f64) / (frames_sent as f64)
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct ThroughputStats {
    pub send_throughput_mbps: f64,
    pub recv_throughput_mbps: f64,
    pub avg_send_latency_us: f64,
    pub avg_recv_latency_us: f64,
    pub buffer_efficiency: f64,
    pub retransmission_rate: f64,
}

/// High-performance buffer pool for reducing allocation overhead
pub struct BufferPool {
    small_buffers: VecDeque<BytesMut>,  // 0-1KB
    medium_buffers: VecDeque<BytesMut>, // 1-16KB
    large_buffers: VecDeque<BytesMut>,  // 16-64KB
    max_pooled_per_size: usize,
    metrics: &'static StreamMetrics,
}

impl BufferPool {
    pub fn new(max_pooled_per_size: usize, metrics: &'static StreamMetrics) -> Self {
        Self {
            small_buffers: VecDeque::with_capacity(max_pooled_per_size),
            medium_buffers: VecDeque::with_capacity(max_pooled_per_size),
            large_buffers: VecDeque::with_capacity(max_pooled_per_size),
            max_pooled_per_size,
            metrics,
        }
    }

    /// Get a buffer of appropriate size from the pool
    pub fn get_buffer(&mut self, size: usize) -> BytesMut {
        let buffer = match size {
            0..=1024 => {
                if let Some(mut buf) = self.small_buffers.pop_front() {
                    buf.clear();
                    if buf.capacity() < size {
                        buf.reserve(size - buf.capacity());
                    }
                    self.metrics.record_buffer_pool_hit();
                    buf
                } else {
                    self.metrics.record_buffer_pool_miss();
                    BytesMut::with_capacity(1024.max(size))
                }
            }
            1025..=16384 => {
                if let Some(mut buf) = self.medium_buffers.pop_front() {
                    buf.clear();
                    if buf.capacity() < size {
                        buf.reserve(size - buf.capacity());
                    }
                    self.metrics.record_buffer_pool_hit();
                    buf
                } else {
                    self.metrics.record_buffer_pool_miss();
                    BytesMut::with_capacity(16384.max(size))
                }
            }
            _ => {
                if let Some(mut buf) = self.large_buffers.pop_front() {
                    buf.clear();
                    if buf.capacity() < size {
                        buf.reserve(size - buf.capacity());
                    }
                    self.metrics.record_buffer_pool_hit();
                    buf
                } else {
                    self.metrics.record_buffer_pool_miss();
                    BytesMut::with_capacity(65536.max(size))
                }
            }
        };

        self.metrics.record_buffer_allocation(buffer.capacity());
        buffer
    }

    /// Return a buffer to the pool for reuse
    pub fn return_buffer(&mut self, buffer: BytesMut) {
        if buffer.capacity() == 0 {
            return;
        }

        let pool = match buffer.capacity() {
            0..=1024 => &mut self.small_buffers,
            1025..=16384 => &mut self.medium_buffers,
            _ => &mut self.large_buffers,
        };

        if pool.len() < self.max_pooled_per_size {
            pool.push_back(buffer);
        }
        // If pool is full, let buffer drop naturally
    }

    /// Get pool statistics
    pub fn get_stats(&self) -> PoolStats {
        PoolStats {
            small_pooled: self.small_buffers.len(),
            medium_pooled: self.medium_buffers.len(),
            large_pooled: self.large_buffers.len(),
            total_capacity: self
                .small_buffers
                .iter()
                .map(|b| b.capacity())
                .sum::<usize>()
                + self
                    .medium_buffers
                    .iter()
                    .map(|b| b.capacity())
                    .sum::<usize>()
                + self
                    .large_buffers
                    .iter()
                    .map(|b| b.capacity())
                    .sum::<usize>(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub small_pooled: usize,
    pub medium_pooled: usize,
    pub large_pooled: usize,
    pub total_capacity: usize,
}

/// Performance timer for measuring operation latencies
pub struct PerfTimer {
    start: Instant,
}

impl PerfTimer {
    #[inline]
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

/// SIMD-optimized memory operations (placeholder for future SIMD implementations)
pub struct SIMDUtils;

impl SIMDUtils {
    /// Fast memory copy using platform-specific optimizations
    #[inline]
    pub fn copy_bytes(src: &[u8], dst: &mut [u8]) {
        // Currently uses standard memcpy, but can be enhanced with SIMD
        let len = src.len().min(dst.len());
        dst[..len].copy_from_slice(&src[..len]);
    }

    /// Fast memory comparison
    #[inline]
    pub fn compare_bytes(a: &[u8], b: &[u8]) -> bool {
        // Currently uses standard comparison, but can be enhanced with SIMD
        a == b
    }

    /// Fast memory fill
    #[inline]
    pub fn fill_bytes(dst: &mut [u8], value: u8) {
        // Currently uses standard fill, but can be enhanced with SIMD
        dst.fill(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_metrics() {
        let metrics = StreamMetrics::new();

        // Test send recording
        metrics.record_send(1024, Duration::from_micros(100));
        assert_eq!(metrics.frames_sent.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.bytes_sent.load(Ordering::Relaxed), 1024);

        // Test buffer pool metrics
        metrics.record_buffer_pool_hit();
        metrics.record_buffer_pool_miss();

        let stats = metrics.get_throughput_stats();
        assert!(stats.buffer_efficiency > 0.0);
    }

    #[test]
    fn test_buffer_pool() {
        let metrics = Box::leak(Box::new(StreamMetrics::new()));
        let mut pool = BufferPool::new(10, metrics);

        // Test buffer allocation
        let buf1 = pool.get_buffer(512);
        assert!(buf1.capacity() >= 512);

        let buf2 = pool.get_buffer(8192);
        assert!(buf2.capacity() >= 8192);

        // Test buffer return and reuse
        pool.return_buffer(buf1);
        let buf3 = pool.get_buffer(256);
        assert!(buf3.capacity() >= 256);

        // Force pool initialization and stats collection
        pool.return_buffer(buf2);
        pool.return_buffer(buf3);

        let stats = pool.get_stats();
        // stats.total_capacity is usize, so always >= 0 by definition
        assert!(stats.total_capacity > 0); // Pool should have some capacity
    }

    #[test]
    fn test_perf_timer() {
        let timer = PerfTimer::start();
        std::thread::sleep(Duration::from_millis(1));
        let elapsed = timer.elapsed();
        assert!(elapsed >= Duration::from_millis(1));
    }
}
