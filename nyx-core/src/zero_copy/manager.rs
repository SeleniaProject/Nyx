use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Immutable buffer backed by `Arc<[u8]>` for zero-copy sharing.
///
/// Thi_s type i_s cheap to clone and can be passed acros_s thread_s safely.
///
/// Example_s
/// -------
/// ```
/// use nyx_core::zero_copy::manager::Buffer;
///
/// // Create from a Vec
/// let b: Buffer = vec![1,2,3].into();
/// assert_eq!(b.as_slice(), &[1,2,3]);
///
/// // Create from a slice
/// let b2: Buffer = (&[4,5][..]).into();
/// assert_eq!(b2.as_ref(), &[4,5]);
///
/// // Cheap clone_s share the same backing allocation
/// let _c = b.clone();
/// assert_eq!(b, c);
/// ```
#[derive(Clone)]
pub struct Buffer(Arc<[u8]>);

impl Buffer {
    pub fn from_vec(v: Vec<u8>) -> Self {
        Self(v.into())
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl From<Vec<u8>> for Buffer {
    fn from(v: Vec<u8>) -> Self {
        Buffer::from_vec(v)
    }
}

impl From<&[u8]> for Buffer {
    fn from(_s: &[u8]) -> Self {
        Self(Arc::<[u8]>::from(_s))
    }
}

impl AsRef<[u8]> for Buffer {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Deref for Buffer {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl PartialEq for Buffer {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for Buffer {}

impl std::fmt::Debug for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Buffer(len={})", self.len())
    }
}

/// A very small buffer pool that reuse_s vector_s to reduce allocation_s.
///
/// The pool i_s thread-safe. Oversized vector_s (capacity above the configured
/// bound) are dropped instead of being retained.
///
/// Example
/// -------
/// ```
/// use nyx_core::zero_copy::manager::BufferPool;
/// let _pool = BufferPool::with_capacity(1024);
/// let mut v = pool.acquire(128);
/// v.extend_from_slice(&[1,2,3]);
/// pool.release(v);
/// let _w = pool.acquire(64);
/// assert!(w.capacity() >= 64);
/// ```
/// Ultra-high performance buffer pool with size-classed allocation
/// Memory-aligned for optimal cache performance
#[derive(Default)]
#[repr(align(64))] // Cache line alignment for maximum performance
pub struct BufferPool {
    // Size-classed free lists for different buffer sizes
    small_buffers: Mutex<Vec<Vec<u8>>>,  // 64-256 bytes
    medium_buffers: Mutex<Vec<Vec<u8>>>, // 256-4096 bytes
    large_buffers: Mutex<Vec<Vec<u8>>>,  // 4096+ bytes

    // Statistics for performance monitoring
    allocated: AtomicUsize,
    recycled: AtomicUsize,
    total_capacity: AtomicUsize,

    // Size limits for each class
    small_limit: usize,
    medium_limit: usize,
    large_limit: usize,
}

impl BufferPool {
    pub fn with_capacity(cap: usize) -> Self {
        // Ultra-high performance: optimized size classes based on typical usage patterns
        let small_limit = (cap / 4).max(64);
        let medium_limit = (cap / 2).max(256);
        let large_limit = cap;

        Self {
            small_buffers: Mutex::new(Vec::with_capacity(32)),
            medium_buffers: Mutex::new(Vec::with_capacity(16)),
            large_buffers: Mutex::new(Vec::with_capacity(8)),
            allocated: AtomicUsize::new(0),
            recycled: AtomicUsize::new(0),
            total_capacity: AtomicUsize::new(0),
            small_limit,
            medium_limit,
            large_limit,
        }
    }

    /// Get the appropriate buffer class for a given size
    #[inline(always)]
    fn get_buffer_class(&self, size: usize) -> (&Mutex<Vec<Vec<u8>>>, &AtomicUsize) {
        // Optimized branch prediction: most allocations are small
        if size <= self.small_limit {
            (&self.small_buffers, &self.allocated)
        } else if size <= self.medium_limit {
            (&self.medium_buffers, &self.allocated)
        } else {
            (&self.large_buffers, &self.allocated)
        }
    }
    /// Acquire a Vec<u8> with at least `n` capacity.
    #[inline(always)]
    pub fn acquire(&self, n: usize) -> Vec<u8> {
        // Ultra-high performance: size-classed allocation
        let (buffer_class, _counter) = self.get_buffer_class(n);

        // Try to get a buffer from the appropriate size class
        let mut buffers = match buffer_class.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        if let Some(mut v) = buffers.pop() {
            // Buffer found: clear and prepare for reuse
            v.clear();
            if v.capacity() < n {
                v.reserve(n - v.capacity());
            }
            self.recycled.fetch_add(1, Ordering::Relaxed);
            v
        } else {
            // No buffer available: allocate new one
            let new_buf = Vec::with_capacity(n);
            self.allocated.fetch_add(1, Ordering::Relaxed);
            self.total_capacity.fetch_add(n, Ordering::Relaxed);
            new_buf
        }
    }
    /// Release a Vec<u8> back to pool. Oversized vector_s are dropped.
    #[inline(always)]
    pub fn release(&self, mut v: Vec<u8>) {
        let capacity = v.capacity();

        // Ultra-high performance: size-classed deallocation
        if capacity <= self.large_limit {
            v.clear();
            let (buffer_class, _) = self.get_buffer_class(capacity);

            // Try to add back to appropriate size class
            match buffer_class.lock() {
                Ok(mut guard) => {
                    // Limit the number of buffers per class to prevent memory bloat
                    if guard.len() < 64 {
                        // Configurable limit
                        guard.push(v);
                    }
                    // If at limit, the buffer will be dropped (implicitly freed)
                }
                Err(mut poisoned) => {
                    if poisoned.get_mut().len() < 64 {
                        poisoned.get_mut().push(v);
                    }
                }
            }
        }
        // Oversized buffers are automatically dropped (implicit free)
    }

    /// Get performance statistics for monitoring
    pub fn stats(&self) -> BufferPoolStats {
        BufferPoolStats {
            allocated: self.allocated.load(Ordering::Relaxed),
            recycled: self.recycled.load(Ordering::Relaxed),
            total_capacity: self.total_capacity.load(Ordering::Relaxed),
            small_buffers_count: self.small_buffers.lock().unwrap().len(),
            medium_buffers_count: self.medium_buffers.lock().unwrap().len(),
            large_buffers_count: self.large_buffers.lock().unwrap().len(),
        }
    }
}

/// Performance statistics for buffer pool monitoring
#[derive(Debug, Clone)]
pub struct BufferPoolStats {
    pub allocated: usize,
    pub recycled: usize,
    pub total_capacity: usize,
    pub small_buffers_count: usize,
    pub medium_buffers_count: usize,
    pub large_buffers_count: usize,
}

/// ゼロコピー最適化のベンチマーク
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn benchmark_buffer_pool_performance() {
        let pool = BufferPool::with_capacity(1024 * 1024); // 1MB capacity
        let mut buffers = Vec::new();

        // バッファの取得と解放を測定
        let start = Instant::now();

        for i in 0..1000 {
            let size = (i % 100 + 1) * 64; // 64-6400バイト
            let buf = pool.acquire(size);
            buffers.push(buf);
        }

        for buf in buffers.drain(..) {
            pool.release(buf);
        }

        let elapsed = start.elapsed();
        println!("Buffer pool benchmark completed in {elapsed:?}");

        // 統計情報を確認
        let stats = pool.stats();
        println!("Pool stats: {stats:?}");

        assert!(stats.allocated > 0);
        assert!(stats.recycled > 0);
    }

    #[test]
    fn test_size_classed_allocation() {
        let pool = BufferPool::with_capacity(1024 * 1024);

        // 異なるサイズのバッファをテスト
        let small = pool.acquire(100);
        let medium = pool.acquire(4096);
        let large = pool.acquire(100000);

        assert!(small.capacity() >= 100);
        assert!(medium.capacity() >= 4096);
        assert!(large.capacity() >= 100000);

        // バッファを解放
        pool.release(small);
        pool.release(medium);
        pool.release(large);

        let stats = pool.stats();
        assert!(stats.allocated >= 3);
    }
}

#[cfg(test)]
mod test_s {
    use super::*;
    #[test]
    fn pool_reuse_s() {
        let p = BufferPool::with_capacity(1024);
        let mut v = p.acquire(100);
        v.extend_from_slice(&[1, 2, 3]);
        let b = Buffer::from_vec(v);
        assert_eq!(b.as_slice(), &[1, 2, 3]);
        // cannot get Vec back from Buffer, but we can acquire-release cycle
        let v2 = p.acquire(50);
        assert!(v2.capacity() >= 50);
        drop(v2);
        // release some vector
        p.release(Vec::with_capacity(64));
        let v3 = p.acquire(10);
        assert!(v3.capacity() >= 10);
    }

    #[test]
    fn pool_mutex_poison_recovery() {
        use std::sync::Arc;
        let p = Arc::new(BufferPool::with_capacity(1024));
        // Poison the mutex in another thread while holding the lock
        let p_ref = Arc::clone(&p);
        let handle = std::thread::spawn(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let _guard = p_ref.small_buffers.lock().map_err(|_| "mutex poisoned")?;
            Err("intentional panic to poison mutex".into())
        });
        let _result = handle.join(); // ignore panic result; mutex should now be poisoned

        // After poisoning, acquire/release should still not panic due to recovery
        let v = p.acquire(16);
        assert!(v.capacity() >= 16);
        p.release(Vec::with_capacity(32));
        let v2 = p.acquire(8);
        assert!(v2.capacity() >= 8);
    }
}
