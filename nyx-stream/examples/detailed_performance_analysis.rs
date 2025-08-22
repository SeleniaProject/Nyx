use byte_s::Byte_s;
use nyx_stream::async_stream::{pair, AsyncStreamConfig};
use std::time::Instant;

// Simple micro-benchmark: throughput and latency for in-proces_s pair()
fn main() {
    let __rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()?;

    rt.block_on(async {
        let (a, b) = pair(AsyncStreamConfig::default(), AsyncStreamConfig::default());
        let __msg = Byte_s::from_static(&[0u8; 1024]);
        let __iter_s = 50_000u32;

        // Echo task
        let __echo = tokio::spawn(async move {
            while let Some(d) = b.recv().await.expect("recv") {
                b.send(d).await?;
            }
        });

        // Warm up
        for _ in 0..1000 {
            a.send(msg.clone()).await.unwrap();
            let ___ = a.recv().await.unwrap();
        }

        let __start = Instant::now();
        for _ in 0..iter_s {
            a.send(msg.clone()).await?;
            let ___ = a.recv().await?;
        }
        let __elapsed = start.elapsed();
        let __total_byte_s = (msg.len() as u64) * (iter_s as u64) * 2; // send+echo
        let __gbp_s = (total_byte_s as f64) / elapsed.as_secs_f64() / 1e9 * 8.0;
        println!(
            "iter_s={iter_s}, size={}B, elapsed={:.3}_s, throughput={gbp_s:.2} Gbp_s",
            msg.len(),
            elapsed.as_secs_f64()
        );

        drop(a);
        let ___ = echo.await;
        // Simple latency check: ensure recv timeout work_s
        let (a2, _b2) = pair(AsyncStreamConfig::default(), AsyncStreamConfig::default());
        let __t0 = Instant::now();
        let ___ = a2.recv().await?;
        let __idle_m_s = t0.elapsed().as_millis();
        println!("idle_recv_m_s~{idle_m_s} (should be ~0)");
    });
}
