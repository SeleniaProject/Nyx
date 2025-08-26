use criterion::{black_box, criterion_group, criterion_main, Criterion};
use nyx_core::performance::RateLimiter;

// Conditional imports based on feature availability
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

#[cfg(not(feature = "zero_copy"))]
fn bench_buffer_pool(c: &mut Criterion) {
    c.bench_function("buffer_pool placeholder", |b| {
        b.iter(|| {
            black_box(42);
        })
    });
}

fn bench_aead_copy_vs_slice(c: &mut Criterion) {
    use rand::Rng;
    
    // Generate test data
    let mut rng = rand::thread_rng();
    let plaintext: Vec<u8> = (0..4096).map(|_| rng.gen()).collect();
    let key: [u8; 32] = rng.gen();
    let nonce: [u8; 12] = rng.gen();
    
    #[cfg(feature = "crypto")]
    {
        use nyx_crypto::aead::{AeadCipher, ChaCha20Poly1305};
        
        let cipher = ChaCha20Poly1305::new(&key);
        
        c.bench_function("aead encrypt copy", |b| {
            b.iter(|| {
                let mut data = plaintext.clone();
                let result = cipher.encrypt_in_place(&nonce, &[], &mut data);
                black_box(result);
            })
        });
        
        let mut ciphertext = plaintext.clone();
        cipher.encrypt_in_place(&nonce, &[], &mut ciphertext).unwrap();
        
        c.bench_function("aead decrypt slice", |b| {
            b.iter(|| {
                let mut data = ciphertext.clone();
                let result = cipher.decrypt_in_place(&nonce, &[], &mut data);
                black_box(result);
            })
        });
    }
    
    #[cfg(not(feature = "crypto"))]
    {
        // Simulate AEAD operations for benchmarking purposes
        c.bench_function("aead encrypt simulation", |b| {
            b.iter(|| {
                let mut data = plaintext.clone();
                // Simulate encryption overhead
                for byte in data.iter_mut() {
                    *byte = byte.wrapping_add(1);
                }
                black_box(data);
            })
        });
        
        c.bench_function("aead decrypt simulation", |b| {
            b.iter(|| {
                let mut data = plaintext.clone();
                // Simulate decryption overhead
                for byte in data.iter_mut() {
                    *byte = byte.wrapping_sub(1);
                }
                black_box(data);
            })
        });
    }
}



fn bench_fec_copy_vs_view(c: &mut Criterion) {
    use rand::Rng;
    
    let mut rng = rand::thread_rng();
    let data: Vec<u8> = (0..8192).map(|_| rng.gen()).collect();
    
    #[cfg(feature = "fec")]
    {
        use nyx_fec::reed_solomon::{ReedSolomonEncoder, ReedSolomonDecoder};
        use nyx_fec::padding::{pack_into_shard, unpack_from_shard};
        
        c.bench_function("fec encode copy", |b| {
            b.iter(|| {
                let encoder = ReedSolomonEncoder::new(16, 8).unwrap();
                let mut shards = Vec::new();
                
                for chunk in data.chunks(1024) {
                    let shard = pack_into_shard(chunk);
                    shards.push(shard);
                }
                
                let result = encoder.encode(&shards);
                black_box(result);
            })
        });
        
        c.bench_function("fec decode view", |b| {
            b.iter(|| {
                let decoder = ReedSolomonDecoder::new(16, 8).unwrap();
                let mut shards = Vec::new();
                
                for chunk in data.chunks(1024) {
                    let shard = pack_into_shard(chunk);
                    shards.push(shard);
                }
                
                // Simulate some missing shards
                if shards.len() > 8 {
                    shards.truncate(16);
                }
                
                let result = decoder.decode(&shards);
                black_box(result);
            })
        });
    }
    
    #[cfg(not(feature = "fec"))]
    {
        // Simulate FEC operations for benchmarking purposes
        c.bench_function("fec encode simulation", |b| {
            b.iter(|| {
                let mut encoded_data = data.clone();
                // Simulate encoding overhead
                encoded_data.extend_from_slice(&data[..data.len() / 2]);
                black_box(encoded_data);
            })
        });
        
        c.bench_function("fec decode simulation", |b| {
            b.iter(|| {
                let encoded_size = data.len() + data.len() / 2;
                let mut encoded_data = Vec::with_capacity(encoded_size);
                encoded_data.extend_from_slice(&data);
                encoded_data.extend_from_slice(&data[..data.len() / 2]);
                
                // Simulate decoding
                let recovered = &encoded_data[..data.len()];
                black_box(recovered);
            })
        });
    }
}


fn bench_rate_limiter(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_limiter_performance");
    
    // Benchmark original allow method
    group.bench_function("allow_standard", |b| {
        b.iter(|| {
            let mut rl = RateLimiter::new(10.0, 10.0);
            let mut cnt = 0;
            for _ in 0..1000 {
                if rl.allow() {
                    cnt += 1;
                }
            }
            black_box(cnt);
        })
    });

    // Benchmark optimized allow method
    group.bench_function("allow_optimized", |b| {
        b.iter(|| {
            let mut rl = RateLimiter::new(10.0, 10.0);
            let mut cnt = 0;
            for _ in 0..1000 {
                if rl.allow_optimized() {
                    cnt += 1;
                }
            }
            black_box(cnt);
        })
    });

    // Benchmark ultra-fast allow method
    group.bench_function("allow_ultra_fast", |b| {
        b.iter(|| {
            let mut rl = RateLimiter::new(10.0, 10.0);
            let mut cnt = 0;
            for _ in 0..1000 {
                if rl.allow_ultra_fast() {
                    cnt += 1;
                }
            }
            black_box(cnt);
        })
    });

    group.finish();
}

// Define benchmark groups based on available features
criterion_group!(
    benches,
    bench_buffer_pool,
    bench_aead_copy_vs_slice,
    bench_fec_copy_vs_view,
    bench_rate_limiter
);

criterion_main!(benches);
