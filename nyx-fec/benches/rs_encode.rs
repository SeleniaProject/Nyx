use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nyx_fec::padding::SHARD_SIZE;
use nyx_fec::rs1280::{Rs1280, RsConfig};

fn bench_rs_encode(c: &mut Criterion) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let cfg = RsConfig {
        data_shards: 8,
        parity_shards: 4,
    };
    let rs = Rs1280::new(cfg)?;

    let mut shards: Vec<[u8; SHARD_SIZE]> = (0..cfg.total_shards())
        .map(|i| {
            let mut a = [0u8; SHARD_SIZE];
            a[0] = i as u8;
            a
        })
        .collect();
    let (data, parity) = shards.split_at_mut(cfg.data_shards);
    let data_refs: Vec<&[u8; SHARD_SIZE]> = data.iter().collect();
    let mut parity_refs: Vec<&mut [u8; SHARD_SIZE]> = parity.iter_mut().collect();

    c.bench_function("rs1280_encode_parity_8p4", |b| {
        b.iter(|| {
            rs.encode_parity(black_box(&data_refs), black_box(&mut parity_refs))
                .unwrap();
        })
    });
    Ok(())
}

fn bench_rs_encode_wrapper(c: &mut Criterion) {
    bench_rs_encode(c).unwrap();
}

criterion_group!(fec, bench_rs_encode_wrapper);
criterion_main!(fec);
