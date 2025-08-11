#![forbid(unsafe_code)]
use nyx_stream::tx::{TxQueue, TimingConfig};
use tracing::subscriber::with_default;
use tracing_subscriber::{Registry, layer::SubscriberExt};
use tracing_subscriber::fmt::format::FmtSpan;
use std::sync::{Arc, Mutex};

struct Buf(Arc<Mutex<String>>);
impl std::io::Write for Buf { fn write(&mut self, b:&[u8])->std::io::Result<usize>{ let mut g=self.0.lock().unwrap(); g.push_str(&String::from_utf8_lossy(b)); Ok(b.len()) } fn flush(&mut self)->std::io::Result<()> { Ok(()) } }
#[test]
fn span_includes_path_and_cid_fields() {
    // Capture tracing output and ensure instrument macro produced fields.
    let store = Arc::new(Mutex::new(String::new()));
    let writer = store.clone();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();
    let layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_span_events(FmtSpan::NEW | FmtSpan::ENTER | FmtSpan::CLOSE)
        .with_ansi(false)
        .with_writer(move || Buf(writer.clone()));
    let subscriber = Registry::default().with(layer);
    with_default(subscriber, || {
        let q = TxQueue::new(TimingConfig::default());
        tokio::runtime::Handle::current().block_on(async { q.send_with_path(7, vec![1,2,3]).await; });
    });
    let logs = store.lock().unwrap().clone();
    assert!(logs.contains("nyx.stream.send"), "expected span name in logs: {:?}", logs);
    assert!(logs.contains("path_id"), "expected path_id attribute present: {}", logs);
    assert!(logs.contains("cid"), "expected cid attribute present: {}", logs);
}
