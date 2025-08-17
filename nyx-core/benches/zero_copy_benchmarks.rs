use criterion::{criterion_group, criterion_main, Criterion, black_box};
use nyx_core::performance::RateLimiter;
#[cfg(feature = "zero_copy")]
use nyx_core::zero_copy::manager::BufferPool;

#[cfg(feature = "zero_copy")]
fn bench_buffer_pool(c: &mut Criterion) {
	let pool = BufferPool::with_capacity(8192);
	c.bench_function("buffer_pool acquire+release 1k", |b| {
		b.iter(|| {
			let mut v = pool.acquire(1024);
			v.extend_from_slice(&[0u8; 1024]);
			black_box(v.len());
			pool.release(v);
		})
	});
}

fn bench_rate_limiter(c: &mut Criterion) {
	c.bench_function("rate_limiter allow loop", |b| {
		b.iter(|| {
			let mut rl = RateLimiter::new(10.0, 10.0);
			let mut cnt = 0;
			for _ in 0..1000 { if rl.allow() { cnt += 1; } }
			black_box(cnt);
		})
	});
}

#[cfg(feature = "zero_copy")]
criterion_group!(benches, bench_buffer_pool, bench_rate_limiter);
#[cfg(not(feature = "zero_copy"))]
criterion_group!(benches, bench_rate_limiter);
criterion_main!(benches);

