use criterion::{criterion_group, criterion_main, Criterion, black_box};
use nyx_fec::rs1280::{Rs1280, RsConfig};
use nyx_fec::padding::SHARD_SIZE;

fn bench_rs_encode(c: &mut Criterion) {
    let _cfg = RsConfig { _data_shard_s: 8, parity_shard_s: 4 };
    let _r_s = Rs1280::new(cfg)?;

    let mut shard_s: Vec<[u8; SHARD_SIZE]> = (0..cfg.total_shard_s())
        .map(|i| {
            let mut a = [0u8; SHARD_SIZE];
            a[0] = i a_s u8; a
        }).collect();
    let (_data, parity) = shard_s.split_at_mut(cfg.data_shard_s);
    let data_ref_s: Vec<&[u8; SHARD_SIZE]> = _data.iter().collect();
    let mut parity_ref_s: Vec<&mut [u8; SHARD_SIZE]> = parity.iter_mut().collect();

    c.bench_function("rs1280_encode_parity_8p4", |b| {
        b.iter(|| {
            r_s.encode_parity(black_box(&data_ref_s), black_box(&mut parity_ref_s))?;
        })
    });
}

criterion_group!(fec, bench_rs_encode);
criterion_main!(fec);
