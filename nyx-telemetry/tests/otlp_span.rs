#![cfg(feature = "otlp")]
use nyx_telemetry::otlp::{force_flush, init_in_memory_tracer};
use tracing::Level;

#[tokio::test(flavor = "current_thread")]
async fn capture_and_flush_spans_in_memory() {
    let (dispatch, store) = init_in_memory_tracer("nyx-test", 1.0);
    tracing::dispatcher::with_default(&dispatch, || {
        let span = tracing::span!(
            Level::INFO,
            "nyx.stream.send",
            path_id = 7u8,
            cid = "cid-otlp"
        );
        let _e = span.enter();
        tracing::info!(target: "nyx", "emit test span");

        // Also emit handshake span to verify capture of nyx.handshake with pq_mode attribute
        let hs = tracing::span!(Level::INFO, "nyx.handshake", pq_mode = "hybrid");
        let _g = hs.enter();
        // drop on scope exit will close the span
    });
    // Allow background to close spans
    force_flush();
    let captured = store.lock().unwrap();
    assert!(captured.iter().any(|s| s.name == "nyx.stream.send"));
    // Verify handshake span presence and pq_mode attribute exists
    let hs = captured.iter().find(|s| s.name == "nyx.handshake");
    assert!(hs.is_some(), "expected nyx.handshake span");
    let hs = hs.unwrap();
    assert_eq!(
        hs.attributes.get("pq_mode").map(|s| s.as_str()),
        Some("hybrid")
    );
}
