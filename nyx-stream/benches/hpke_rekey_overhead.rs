use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, black_box};
use std::time::{Duration, Instant};
use nyx_stream::{hpke_rekey_manager::{HpkeRekeyManager, RekeyPolicy}, hpke_rekey_manager::RekeyDecision};
use nyx_crypto::noise::SessionKey;

fn bench_rekey_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("hpke_rekey_overhead");
    // 異なる min_cooldown / rekey頻度 (packet_interval) 組合せを比較
    for &(min_cd_ms, pkt_interval) in &[(0u64, 10u64), (10, 10), (50, 10), (10, 100), (50,100)] {
        group.bench_with_input(BenchmarkId::from_parameter(format!("cd{}ms_pkt{}", min_cd_ms, pkt_interval)), &(min_cd_ms, pkt_interval), |b, &(m,p)| {
            b.iter(|| {
                let policy = RekeyPolicy { time_interval: Duration::from_secs(1000), packet_interval: p, grace_period: Duration::from_millis(5), min_cooldown: Duration::from_millis(m) };
                let mut mgr = HpkeRekeyManager::new(policy, SessionKey([0u8;32]));
                let mut initiated = 0u64;
                let mut applied = 0u64;
                for i in 0..500u64 { // simulate 500 packets
                    if matches!(mgr.on_packet_sent(), RekeyDecision::Initiate) {
                        initiated += 1;
                        // pretend new key produced
                        mgr.install_new_key(SessionKey([(i%255) as u8;32]));
                        applied += 1;
                    }
                }
                black_box((initiated, applied));
            });
        });
    }
    group.finish();
}

criterion_group!(name = hpke; config = Criterion::default().sample_size(30); targets = bench_rekey_overhead);
criterion_main!(hpke);
