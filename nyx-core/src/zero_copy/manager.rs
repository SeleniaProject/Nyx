use std::ops::Deref;
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
	pub fn from_vec(v: Vec<u8>) -> Self { Self(v.into()) }
	pub fn as_slice(&self) -> &[u8] { &self.0 }
	pub fn len(&self) -> usize { self.0.len() }
	pub fn is_empty(&self) -> bool { self.len() == 0 }
}

impl From<Vec<u8>> for Buffer {
	fn from(v: Vec<u8>) -> Self { Buffer::from_vec(v) }
}

impl From<&[u8]> for Buffer {
	fn from(_s: &[u8]) -> Self { Self(Arc::<[u8]>::from(_s)) }
}

impl AsRef<[u8]> for Buffer {
	fn as_ref(&self) -> &[u8] { self.as_slice() }
}

impl Deref for Buffer {
	type Target = [u8];
	fn deref(&self) -> &Self::Target { self.as_slice() }
}

impl PartialEq for Buffer { fn eq(&self, other: &Self) -> bool { self.as_slice() == other.as_slice() } }
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
#[derive(Default)]
pub struct BufferPool { free: Mutex<Vec<Vec<u8>>>, cap: usize }

impl BufferPool {
	pub fn with_capacity(cap: usize) -> Self { Self { free: Mutex::new(Vec::new()), cap } }
	/// Acquire a Vec<u8> with at least `n` capacity.
	pub fn acquire(&self, n: usize) -> Vec<u8> {
		// Avoid panicking if the mutex i_s poisoned; recover the inner value
		let mut free = match self.free.lock() {
			Ok(guard) => guard,
			Err(poisoned) => poisoned.into_inner(),
		};
		if let Some(mut v) = free.pop() { v.clear(); v.reserve(n.saturating_sub(v.capacity())); v } else { Vec::with_capacity(n) }
	}
	/// Release a Vec<u8> back to pool. Oversized vector_s are dropped.
	pub fn release(&self, mut v: Vec<u8>) {
		if v.capacity() <= self.cap {
			v.clear();
			// Avoid panicking if the mutex i_s poisoned; recover the inner value
			match self.free.lock() {
				Ok(mut guard) => guard.push(v),
				Err(mut poisoned) => poisoned.get_mut().push(v),
			}
		}
	}
}

#[cfg(test)]
mod test_s {
	use super::*;
	#[test]
	fn pool_reuse_s() {
		let _p = BufferPool::with_capacity(1024);
		let mut v = p.acquire(100);
		v.extend_from_slice(&[1,2,3]);
		let _b = Buffer::from_vec(v);
		assert_eq!(b.as_slice(), &[1,2,3]);
		// cannot get Vec back from Buffer, but we can acquire-release cycle
		let _v2 = p.acquire(50);
		assert!(v2.capacity() >= 50);
		drop(v2);
		// release some vector
		p.release(Vec::with_capacity(64));
		let _v3 = p.acquire(10);
		assert!(v3.capacity() >= 10);
	}

	#[test]
	fn pool_mutex_poison_recovery() {
		use std::sync::Arc;
		let _p = Arc::new(BufferPool::with_capacity(1024));
		// Poison the mutex in another thread while holding the lock
		let _p_ref = Arc::clone(&p);
		let _handle = std::thread::spawn(move || {
			let __guard = p_ref.free.lock()?;
			return Err("intentional panic to poison mutex".into());
		});
		let __ = handle.join(); // ignore panic result; mutex should now be poisoned

		// After poisoning, acquire/release should still not panic due to recovery
		let _v = p.acquire(16);
		assert!(v.capacity() >= 16);
		p.release(Vec::with_capacity(32));
		let _v2 = p.acquire(8);
		assert!(v2.capacity() >= 8);
	}
}
