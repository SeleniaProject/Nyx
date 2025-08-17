use criterion::{criterion_group, criterion_main, Criterion, black_box};
use nyx_core::performance::RateLimiter;
#[cfg(feature = "zero_copy")]
use nyx_core::zero_copy::manager::BufferPool;
#[cfg(feature = "zero_copy")]
use nyx_core::zero_copy::manager::Buffer;
#[cfg(feature = "zero_copy")]
use rand::{Rng, SeedableRng};
#[cfg(feature = "zero_copy")]
use rand::rngs::StdRng;
#[cfg(feature = "zero_copy")]
use nyx_crypto::aead::{AeadKey, AeadCipher, AeadNonce};

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

#[cfg(feature = "zero_copy")]
fn bench_aead_copy_vs_slice(c: &mut Criterion) {
	// Prepare AEAD cipher with a fixed key
	let key = AeadKey([7u8; 32]);
	let cipher = AeadCipher::new(key);
	let aad = b"bench-aad";
	let mut rng = StdRng::seed_from_u64(42);

	// Prepare a 64 KiB buffer once
	let mut data = vec![0u8; 64 * 1024];
	rng.fill(&mut data[..]);
	let buf: Buffer = data.into();

	// Copy-heavy: allocate and copy per-iter
	c.bench_function("aead seal (copy input vec)", |b| {
		b.iter(|| {
			let mut v = Vec::with_capacity(buf.len());
			v.extend_from_slice(buf.as_slice());
			let _ct = cipher.seal(AeadNonce([0u8;12]), aad, &v).unwrap();
		})
	});

	// Zero-copy-ish: pass slice directly
	c.bench_function("aead seal (slice from Buffer)", |b| {
		b.iter(|| {
			let _ct = cipher.seal(AeadNonce([0u8;12]), aad, buf.as_slice()).unwrap();
		})
	});
}

#[cfg(all(feature = "zero_copy", feature = "fec"))]
fn bench_fec_copy_vs_view(c: &mut Criterion) {
	use nyx_fec::{padding::SHARD_SIZE, rs1280::{Rs1280, RsConfig}};
	// Construct ~2.5 shards of data
	let mut data = vec![0u8; SHARD_SIZE * 2 + SHARD_SIZE / 2];
	for (i, b) in data.iter_mut().enumerate() { *b = (i % 251) as u8; }
	let buf: Buffer = data.into();

	let cfg = RsConfig { data_shards: 3, parity_shards: 2 };
	let rs = Rs1280::new(cfg).unwrap();

	// Copy-heavy: materialize exact-sized shard arrays every iter
	c.bench_function("fec parity encode (copy shards)", |b| {
		b.iter(|| {
			let mut d0 = [0u8; SHARD_SIZE];
			let mut d1 = [0u8; SHARD_SIZE];
			let mut d2 = [0u8; SHARD_SIZE];
			let bytes = buf.as_slice();
			d0.copy_from_slice(&bytes[..SHARD_SIZE]);
			d1.copy_from_slice(&bytes[SHARD_SIZE..SHARD_SIZE*2]);
			let rem = &bytes[SHARD_SIZE*2..];
			d2[..rem.len()].copy_from_slice(rem);
			let data: [&[u8; SHARD_SIZE]; 3] = [&d0, &d1, &d2];
			let mut p0 = [0u8; SHARD_SIZE];
			let mut p1 = [0u8; SHARD_SIZE];
			let mut parity = [&mut p0, &mut p1];
			rs.encode_parity(&data, &mut parity).unwrap();
			black_box(parity[0][0]);
		})
	});

	// Zero-copy view: reuse slices, copy only the last partial shard into temp
	c.bench_function("fec parity encode (zero-copy view)", |b| {
		use nyx_core::zero_copy::integration::fec_views::shard_view;
		b.iter(|| {
			let shards = shard_view(&buf);
			let d0: &[u8; SHARD_SIZE] = shards[0].try_into().unwrap();
			let d1: &[u8; SHARD_SIZE] = shards[1].try_into().unwrap();
			let mut tmp = [0u8; SHARD_SIZE];
			tmp[..shards[2].len()].copy_from_slice(shards[2]);
			let d2: &[u8; SHARD_SIZE] = &tmp;
			let data: [&[u8; SHARD_SIZE]; 3] = [d0, d1, d2];
			let mut p0 = [0u8; SHARD_SIZE];
			let mut p1 = [0u8; SHARD_SIZE];
			let mut parity = [&mut p0, &mut p1];
			rs.encode_parity(&data, &mut parity).unwrap();
			black_box(parity[0][0]);
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

#[cfg(all(feature = "zero_copy", feature = "fec"))]
criterion_group!(benches, bench_buffer_pool, bench_aead_copy_vs_slice, bench_fec_copy_vs_view, bench_rate_limiter);
#[cfg(all(feature = "zero_copy", not(feature = "fec")))]
criterion_group!(benches, bench_buffer_pool, bench_aead_copy_vs_slice, bench_rate_limiter);
#[cfg(not(feature = "zero_copy"))]
criterion_group!(benches, bench_rate_limiter);
criterion_main!(benches);

