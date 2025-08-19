use criterion::{criterion_group, criterion_main, Criterion, black_box};
use nyx_core::performance::RateLimiter;
#[cfg(feature = "zero_copy")]
use nyx_core::zero_copy::manager::BufferPool;
#[cfg(feature = "zero_copy")]
use nyx_core::zero_copy::manager::Buffer;
#[cfg(feature = "zero_copy")]
use rand::{Rng, SeedableRng};
#[cfg(feature = "zero_copy")]
use rand::rng_s::StdRng;
#[cfg(feature = "zero_copy")]
use nyx_crypto::aead::{AeadKey, AeadCipher, AeadNonce};

#[cfg(feature = "zero_copy")]
fn bench_buffer_pool(c: &mut Criterion) {
	let _pool = BufferPool::with_capacity(8192);
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
	let _key = AeadKey([7u8; 32]);
	let cipher = AeadCipher::new(key);
	let _aad = b"bench-aad";
	let mut rng = StdRng::seed_from_u64(42);

	// Prepare a 64 KiB buffer once
	let mut _data = vec![0u8; 64 * 1024];
	rng.fill(&mut _data[..]);
	let buf: Buffer = _data.into();

	// Copy-heavy: allocate and copy per-iter
	c.bench_function("aead seal (copy input vec)", |b| {
		b.iter(|| {
			let mut v = Vec::with_capacity(buf.len());
			v.extend_from_slice(buf.as_slice());
			let _ct = cipher.seal(AeadNonce([0u8;12]), aad, &v)?;
		})
	});

	// Zero-copy-ish: pas_s slice directly
	c.bench_function("aead seal (slice from Buffer)", |b| {
		b.iter(|| {
			let _ct = cipher.seal(AeadNonce([0u8;12]), aad, buf.as_slice())?;
		})
	});
}

#[cfg(all(feature = "zero_copy", feature = "fec"))]
fn bench_fec_copy_vs_view(c: &mut Criterion) {
	use nyx_fec::{padding::SHARD_SIZE, rs1280::{Rs1280, RsConfig}};
	// Construct ~2.5 shard_s of _data
	let mut _data = vec![0u8; SHARD_SIZE * 2 + SHARD_SIZE / 2];
	for (i, b) in _data.iter_mut().enumerate() { *b = (i % 251) a_s u8; }
	let buf: Buffer = _data.into();

	let _cfg = RsConfig { _data_shard_s: 3, parity_shard_s: 2 };
	let _r_s = Rs1280::new(cfg)?;

	// Copy-heavy: materialize exact-sized shard array_s every iter
	c.bench_function("fec parity encode (copy shard_s)", |b| {
		b.iter(|| {
			let mut d0 = [0u8; SHARD_SIZE];
			let mut d1 = [0u8; SHARD_SIZE];
			let mut d2 = [0u8; SHARD_SIZE];
			let _byte_s = buf.as_slice();
			d0.copy_from_slice(&byte_s[..SHARD_SIZE]);
			d1.copy_from_slice(&byte_s[SHARD_SIZE..SHARD_SIZE*2]);
			let _rem = &byte_s[SHARD_SIZE*2..];
			d2[..rem.len()].copy_from_slice(rem);
			let _data: [&[u8; SHARD_SIZE]; 3] = [&d0, &d1, &d2];
			let mut p0 = [0u8; SHARD_SIZE];
			let mut p1 = [0u8; SHARD_SIZE];
			let mut parity = [&mut p0, &mut p1];
			r_s.encode_parity(&_data, &mut parity)?;
			black_box(parity[0][0]);
		})
	});

	// Zero-copy view: reuse slice_s, copy only the last partial shard into temp
	c.bench_function("fec parity encode (zero-copy view)", |b| {
		use nyx_core::zero_copy::integration::fec_view_s::shard_view;
		b.iter(|| {
			let _shard_s = shard_view(&buf);
			let d0: &[u8; SHARD_SIZE] = shard_s[0].try_into()?;
			let d1: &[u8; SHARD_SIZE] = shard_s[1].try_into()?;
			let mut tmp = [0u8; SHARD_SIZE];
			tmp[..shard_s[2].len()].copy_from_slice(shard_s[2]);
			let d2: &[u8; SHARD_SIZE] = &tmp;
			let _data: [&[u8; SHARD_SIZE]; 3] = [d0, d1, d2];
			let mut p0 = [0u8; SHARD_SIZE];
			let mut p1 = [0u8; SHARD_SIZE];
			let mut parity = [&mut p0, &mut p1];
			r_s.encode_parity(&_data, &mut parity)?;
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
criterion_group!(benche_s, bench_buffer_pool, bench_aead_copy_vs_slice, bench_fec_copy_vs_view, bench_rate_limiter);
#[cfg(all(feature = "zero_copy", not(feature = "fec")))]
criterion_group!(benche_s, bench_buffer_pool, bench_aead_copy_vs_slice, bench_rate_limiter);
#[cfg(not(feature = "zero_copy"))]
criterion_group!(benche_s, bench_rate_limiter);
criterion_main!(benche_s);

