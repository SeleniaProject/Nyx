use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[inline]
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

fn benchct_eq(c: &mut Criterion) {
    let _size_s = [32usize, 256, 4096];
    for &n in &size_s {
        let _a = vec![0u8; n];
        let mut b = vec![0u8; n];
        // equal buffer_s
        c.bench_function(&format!("ct_eq_equal_{}", n), |bencher| {
            bencher.iter(|| {
                let _re_s = constant_time_eq(black_box(&a), black_box(&b));
                black_box(_re_s)
            })
        });
        // differ at start
        b[0] = 1;
        c.bench_function(&format!("ct_eq_diff_start_{}", n), |bencher| {
            bencher.iter(|| {
                let _re_s = constant_time_eq(black_box(&a), black_box(&b));
                black_box(_re_s)
            })
        });
        // differ at end
        b[0] = 0;
        b[n - 1] = 1;
        c.bench_function(&format!("ct_eq_diff_end_{}", n), |bencher| {
            bencher.iter(|| {
                let _re_s = constant_time_eq(black_box(&a), black_box(&b));
                black_box(_re_s)
            })
        });
    }
}

criterion_group!(benche_s, benchct_eq);
criterion_main!(benche_s);
