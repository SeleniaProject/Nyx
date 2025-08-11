use nyx_stream::StreamLayer;
use nyx_stream::tx::TimingConfig; // use stream layer timing config
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn streamlayer_send_recv_roundtrip() {
    // Use small delay to keep test fast.
    let cfg = TimingConfig { mean_ms: 1.0, sigma_ms: 0.0 };
    let mut layer = StreamLayer::new(cfg);
    let payload = vec![0xAA, 0xBB, 0xCC];
    layer.send(payload.clone()).await;

    // Expect to receive same bytes within 100ms after obfuscation delay.
    let received = timeout(Duration::from_millis(100), layer.recv())
        .await
        .expect("timeout")
        .expect("no data");
    assert_eq!(received, payload);
}

#[tokio::test]
async fn streamlayer_inorder_across_paths() {
    let cfg = TimingConfig { mean_ms: 0.0, sigma_ms: 0.0 };
    let mut layer = StreamLayer::new(cfg);
    // MultipathReceiver initializes expected seq to first observed, so first packet delivers immediately.
    let r1 = layer.handle_incoming(2, 1, vec![1]);
    assert_eq!(r1, vec![vec![1]]);
    // A retroactive earlier sequence (< next_seq) is treated as duplicate/old -> ignored (empty result)
    let r2 = layer.handle_incoming(2, 0, vec![0]);
    assert!(r2.is_empty());
} 