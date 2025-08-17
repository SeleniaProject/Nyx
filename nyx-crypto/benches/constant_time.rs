use criterion::{criterion_group, criterion_main, Criterion, black_box};

#[inline]
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
	if a.len() != b.len() { return false; }
	let mut diff = 0u8;
	for i in 0..a.len() { diff |= a[i] ^ b[i]; }
	diff == 0
}

fn bench_ct_eq(c: &mut Criterion) {
	let sizes = [32usize, 256, 4096];
	for &n in &sizes {
		let a = vec![0u8; n];
		let mut b = vec![0u8; n];
		// equal buffers
		c.bench_function(&format!("ct_eq_equal_{}", n), |bencher| {
			bencher.iter(|| {
				let res = constant_time_eq(black_box(&a), black_box(&b));
				black_box(res)
			})
		});
		// differ at start
		b[0] = 1;
		c.bench_function(&format!("ct_eq_diff_start_{}", n), |bencher| {
			bencher.iter(|| {
				let res = constant_time_eq(black_box(&a), black_box(&b));
				black_box(res)
			})
		});
		// differ at end
		b[0] = 0; b[n-1] = 1;
		c.bench_function(&format!("ct_eq_diff_end_{}", n), |bencher| {
			bencher.iter(|| {
				let res = constant_time_eq(black_box(&a), black_box(&b));
				black_box(res)
			})
		});
	}
}

criterion_group!(benches, bench_ct_eq);
criterion_main!(benches);


