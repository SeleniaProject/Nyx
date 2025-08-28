use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use nyx_transport::{tcp_fallback, UdpEndpoint};
// use std::net::SocketAddr; // Currently unused
use std::time::Duration;

fn udp_loopback_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("udp_loopback");

    // Test different message sizes
    let message_sizes = [64, 512, 1024, 4096, 8192];

    for size in message_sizes {
        group.bench_with_input(BenchmarkId::new("send_recv", size), &size, |b, &size| {
            let mut endpoint_a = UdpEndpoint::bind_loopback().unwrap();
            let mut endpoint_b = UdpEndpoint::bind_loopback().unwrap();
            let addr_b = endpoint_b.local_addr().unwrap();

            let message = vec![0u8; size];
            let mut buffer = vec![0u8; size + 1024]; // Extra space for safety

            b.iter(|| {
                // Send message
                endpoint_a
                    .send_to_buffered(black_box(&message), addr_b)
                    .unwrap();

                // Receive message
                let (bytes_recv, _from) = endpoint_b.recv_from(black_box(&mut buffer)).unwrap();
                assert_eq!(bytes_recv, size);
            });
        });
    }

    group.finish();
}

fn udp_buffered_vs_direct(c: &mut Criterion) {
    let mut group = c.benchmark_group("udp_buffered_vs_direct");

    let message = vec![42u8; 1024];

    group.bench_function("direct_send", |b| {
        let mut endpoint_a = UdpEndpoint::bind_loopback().unwrap();
        let endpoint_b = UdpEndpoint::bind_loopback().unwrap();
        let addr_b = endpoint_b.local_addr().unwrap();

        b.iter(|| {
            endpoint_a.send_to(black_box(&message), addr_b).unwrap();
        });
    });

    group.bench_function("buffered_send", |b| {
        let mut endpoint_a = UdpEndpoint::bind_loopback().unwrap();
        let endpoint_b = UdpEndpoint::bind_loopback().unwrap();
        let addr_b = endpoint_b.local_addr().unwrap();

        b.iter(|| {
            endpoint_a
                .send_to_buffered(black_box(&message), addr_b)
                .unwrap();
        });
    });

    group.finish();
}

fn tcp_connection_pool_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("tcp_connection_pool");

    // Setup a simple echo server
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let server_addr = listener.local_addr().unwrap();

    let _server_handle = std::thread::spawn(move || {
        while let Ok((_stream, _addr)) = listener.accept() {
            // Just accept connections for benchmarking
        }
    });

    std::thread::sleep(Duration::from_millis(10)); // Let server start

    group.bench_function("simple_connect", |b| {
        b.iter(|| {
            let _result =
                tcp_fallback::try_connect(black_box(server_addr), Duration::from_millis(100));
        });
    });

    group.bench_function("pooled_connect", |b| {
        let pool = tcp_fallback::create_default_pool();

        b.iter(|| {
            if let Ok(conn) = pool.get_connection(black_box(server_addr)) {
                let _ = pool.return_connection(server_addr, conn);
            }
        });
    });

    group.finish();
}

fn udp_endpoint_stats_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("udp_endpoint_stats");

    group.bench_function("get_stats", |b| {
        let endpoint = UdpEndpoint::bind_loopback().unwrap();

        b.iter(|| {
            let _stats = endpoint.get_stats();
        });
    });

    group.finish();
}

fn tcp_pool_stats_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("tcp_pool_stats");

    group.bench_function("get_stats", |b| {
        let pool = tcp_fallback::create_default_pool();

        b.iter(|| {
            let _stats = pool.get_stats();
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    udp_loopback_benchmark,
    udp_buffered_vs_direct,
    tcp_connection_pool_benchmark,
    udp_endpoint_stats_benchmark,
    tcp_pool_stats_benchmark
);
criterion_main!(benches);
