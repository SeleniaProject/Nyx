
// On Unix, run a simple UDP send benchmark using criterion.
// On non-Unix (e.g., Windows), provide a dummy main so the bench compiles.

#[cfg(unix)]
mod bench {
	use criterion::{criterion_group, criterion_main, Criterion};
	use std::net::UdpSocket;

	fn udp_send_bench(c: &mut Criterion) {
		let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
		let target = "127.0.0.1:9"; // discard; UDP send_to succeeds regardless of listener
		let payload = [0u8; 1200];
		c.bench_function("udp_send_loopback", |b| {
			b.iter(|| {
				let _ = sock.send_to(&payload, target).unwrap();
			})
		});
	}

	criterion_group!(benches, udp_send_bench);
	criterion_main!(benches);
}

#[cfg(not(unix))]
fn main() {}

