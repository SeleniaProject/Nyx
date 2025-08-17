
use std::time::Instant;
use bytes::Bytes;
use nyx_stream::async_stream::{pair, AsyncStreamConfig};

// Simple micro-benchmark: throughput and latency for in-process pair()
fn main() {
	let rt = tokio::runtime::Builder::new_current_thread()
		.enable_time()
		.build()
		.expect("rt");

	rt.block_on(async {
		let (a, b) = pair(AsyncStreamConfig::default(), AsyncStreamConfig::default());
		let msg = Bytes::from_static(&[0u8; 1024]);
		let iters = 50_000u32;

		// Echo task
		let echo = tokio::spawn(async move {
			while let Some(d) = b.recv().await.expect("recv") {
				b.send(d).await.expect("send");
			}
		});

		// Warm up
		for _ in 0..1000 { a.send(msg.clone()).await.unwrap(); let _ = a.recv().await.unwrap(); }

		let start = Instant::now();
		for _ in 0..iters {
			a.send(msg.clone()).await.unwrap();
			let _ = a.recv().await.unwrap();
		}
		let elapsed = start.elapsed();
		let total_bytes = (msg.len() as u64) * (iters as u64) * 2; // send+echo
		let gbps = (total_bytes as f64) / elapsed.as_secs_f64() / 1e9 * 8.0;
		println!("iters={iters}, size={}B, elapsed={:.3}s, throughput={gbps:.2} Gbps", msg.len(), elapsed.as_secs_f64());

		drop(a);
		let _ = echo.await;
		// Simple latency check: ensure recv timeout works
		let (a2, _b2) = pair(AsyncStreamConfig::default(), AsyncStreamConfig::default());
		let t0 = Instant::now();
		let _ = a2.recv().await.expect("recv option");
		let idle_ms = t0.elapsed().as_millis();
		println!("idle_recv_ms~{idle_ms} (should be ~0)");
	});
}

