#![cfg(feature = "otlp")]
//! Integration test validating tracing -> OTLP (in-memory exporter) attribute propagation.
//! This test is feature-gated so default (prometheus-only) builds skip heavy deps.

use nyx_telemetry::record_stream_send; // reused to ensure hook path does not panic under otlp

// We must access the in-memory tracer initializer. Keeping path explicit to avoid wildcard imports.
use nyx_telemetry::otlp::{init_in_memory_tracer};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn span_attributes_propagate_to_in_memory_exporter() {
    // Install in-memory tracer capturing spans.
    let (dispatch, spans) = init_in_memory_tracer("nyx-otlp-test", 1.0);

    // Emit a span consistent with production naming convention.
    tracing::dispatcher::with_default(&dispatch, || {
        let span = tracing::span!(tracing::Level::INFO, "nyx.stream.send", path_id = 5u8, cid = "cid-otlp-test");
        let _e = span.enter();
        tracing::info!("otlp test body");
        record_stream_send(5, "cid-otlp-test");
    });

    // No flush needed; capture occurs on close.

    let store = spans.lock().unwrap();
    assert!(!store.is_empty(), "no spans captured by in-memory exporter");
    let exported = store.iter().find(|s| s.name == "nyx.stream.send").expect("missing expected span");
    assert!(exported.attributes.contains_key("path_id"));
    assert!(exported.attributes.contains_key("cid"));
}
