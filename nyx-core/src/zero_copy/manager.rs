use std::sync::{Arc, Mutex};

/// Immutable buffer backed by Arc for zero-copy sharing.
#[derive(Clone)]
pub struct Buffer(Arc<[u8]>);

impl Buffer {
	pub fn from_vec(v: Vec<u8>) -> Self { Self(v.into()) }
	pub fn as_slice(&self) -> &[u8] { &self.0 }
	pub fn len(&self) -> usize { self.0.len() }
	pub fn is_empty(&self) -> bool { self.len() == 0 }
}

/// A very small buffer pool that reuses vectors to reduce allocations.
#[derive(Default)]
pub struct BufferPool { free: Mutex<Vec<Vec<u8>>>, cap: usize }

impl BufferPool {
	pub fn with_capacity(cap: usize) -> Self { Self { free: Mutex::new(Vec::new()), cap } }
	/// Acquire a Vec<u8> with at least `n` capacity.
	pub fn acquire(&self, n: usize) -> Vec<u8> {
		// Avoid panicking if the mutex is poisoned; recover the inner value
		let mut free = match self.free.lock() {
			Ok(guard) => guard,
			Err(poisoned) => poisoned.into_inner(),
		};
		if let Some(mut v) = free.pop() { v.clear(); v.reserve(n.saturating_sub(v.capacity())); v } else { Vec::with_capacity(n) }
	}
	/// Release a Vec<u8> back to pool. Oversized vectors are dropped.
	pub fn release(&self, mut v: Vec<u8>) {
		if v.capacity() <= self.cap {
			v.clear();
			// Avoid panicking if the mutex is poisoned; recover the inner value
			match self.free.lock() {
				Ok(mut guard) => guard.push(v),
				Err(mut poisoned) => poisoned.get_mut().push(v),
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn pool_reuses() {
		let p = BufferPool::with_capacity(1024);
		let mut v = p.acquire(100);
		v.extend_from_slice(&[1,2,3]);
		let b = Buffer::from_vec(v);
		assert_eq!(b.as_slice(), &[1,2,3]);
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
		let p = BufferPool::with_capacity(1024);
		// Poison the mutex in another thread while holding the lock
		let p_ref = &p;
		let handle = std::thread::spawn(move || {
			let _guard = p_ref.free.lock().expect("lock before poison");
			panic!("intentional panic to poison mutex");
		});
		let _ = handle.join(); // ignore panic result; mutex should now be poisoned

		// After poisoning, acquire/release should still not panic due to recovery
		let v = p.acquire(16);
		assert!(v.capacity() >= 16);
		p.release(Vec::with_capacity(32));
		let v2 = p.acquire(8);
		assert!(v2.capacity() >= 8);
	}
}
