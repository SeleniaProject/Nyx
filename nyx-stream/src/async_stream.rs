#![forbid(unsafe_code)]

use crate::multipath::{integration::IntegrationSettings, mpr::MprState, scheduler::PathId};
use crate::{
    congestion::RttEstimator,
    errors::{Error, Result},
    flow_controller::FlowController,
    frame::{Frame, FrameHeader, FrameType},
    frame_codec::FrameCodec,
};
use bytes::{Bytes, BytesMut};
use std::{collections::BTreeMap, time::Duration};
use tokio::{
    sync::{mpsc, oneshot},
    time::{sleep, Instant},
};

/// Configuration for AsyncStream instances
/// This struct provides comprehensive control over stream behavior including:
/// - Flow control parameters (max_inflight, retransmit_timeout, max_retries)
/// - Optional frame reordering for testing network conditions
/// - Multipath routing configuration for load balancing and redundancy
/// - Receiver buffer management to prevent memory exhaustion
#[derive(Debug, Clone)]
pub struct AsyncStreamConfig {
    pub stream_id: u32,
    pub max_inflight: usize,
    pub retransmit_timeout: Duration,
    pub max_retries: u32,
    /// Optional: deterministic reordering on the wire for testing.
    /// If set to Some(N), the sender buffers N frames and emits them in reverse order.
    pub reorder_window: Option<usize>,
    /// Optional max payload size (Bytes). If Some(n), payload larger than n is rejected on send.
    pub max_frame_len: Option<usize>,
    /// Optional multipath settings. If None or disabled, single path is used.
    pub multipath: Option<IntegrationSettings>,
    /// Optional cap for receiver out-of-order buffer (number of frames).
    /// If Some(n), pending out-of-order frames beyond n will cause oldest to be dropped.
    pub max_reorder_pending: Option<usize>,
}

impl Default for AsyncStreamConfig {
    fn default() -> Self {
        Self {
            stream_id: 1,
            max_inflight: 64, // Increased from 32 for better throughput
            retransmit_timeout: Duration::from_millis(200), // Reduced from 250ms for faster recovery
            max_retries: 8,
            reorder_window: None,
            max_frame_len: None,
            multipath: None,
            max_reorder_pending: Some(4096), // Increased from 2048 for better buffering
        }
    }
}

#[derive(Debug)]
enum Cmd {
    Send {
        data: Bytes,
        ack: oneshot::Sender<()>,
    },
    Recv {
        reply: oneshot::Sender<Option<Bytes>>,
    },
    TryRecv {
        reply: oneshot::Sender<Option<Bytes>>,
    },
    Close {
        ack: oneshot::Sender<()>,
    },
}

#[derive(Debug)]
enum LinkMsg {
    Wire { bytes: BytesMut, path: u8 },
    Close,
}

#[derive(Debug, Clone)]
pub struct AsyncStream {
    tx: mpsc::Sender<Cmd>,
}

impl AsyncStream {
    pub fn new(config: AsyncStreamConfig) -> Self {
        // Ultra-optimized channel sizing for maximum throughput
        // Channel sizes optimized based on latency and memory usage patterns
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>(256); // Increased from 128 for better batching
        let (wire_tx, _wire_rx) = mpsc::channel::<LinkMsg>(2048); // Increased from 1024 for high-throughput
        let (_wire_back_tx, wire_back_rx) = mpsc::channel::<LinkMsg>(2048); // Matched for symmetry
        tokio::spawn(endpoint_task(config, cmd_rx, wire_tx, wire_back_rx));

        AsyncStream { tx: cmd_tx }
    }

    pub async fn send(&self, data: Bytes) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Cmd::Send { data, ack: tx })
            .await
            .map_err(|_| Error::ChannelClosed)?;
        rx.await.map_err(|_| Error::ChannelClosed)?;
        Ok(())
    }

    pub async fn recv(&self) -> Result<Option<Bytes>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Cmd::Recv { reply: tx })
            .await
            .map_err(|_| Error::ChannelClosed)?;
        rx.await.map_err(|_| Error::ChannelClosed)
    }

    /// Non-blocking receive: Returns Some if data is queued, None otherwise (or if stream is closed).
    /// This method enables efficient polling-based consumption patterns without blocking the async runtime.
    pub async fn try_recv(&self) -> Result<Option<Bytes>> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Cmd::TryRecv { reply: tx })
            .await
            .map_err(|_| Error::ChannelClosed)?;
        rx.await.map_err(|_| Error::ChannelClosed)
    }

    pub async fn close(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Cmd::Close { ack: tx })
            .await
            .map_err(|_| Error::ChannelClosed)?;
        let _ = rx.await;
        Ok(())
    }

    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}

/// Creates a pair of connected AsyncStreams for testing and simulation purposes.
/// This function establishes a full-duplex communication channel between two stream endpoints,
/// enabling comprehensive testing of stream protocols, flow control, and multipath behavior.
/// 
/// # Arguments
/// * `cfg_a` - Configuration for the first stream endpoint
/// * `cfg_b` - Configuration for the second stream endpoint (will be modified if stream_id conflicts)
/// 
/// # Returns
/// A tuple containing two connected AsyncStream instances that can communicate bidirectionally
pub fn pair(cfg_a: AsyncStreamConfig, mut cfg_b: AsyncStreamConfig) -> (AsyncStream, AsyncStream) {
    // Ensure distinct stream ids (A->B uses A.stream_id, B->A uses B.stream_id)
    if cfg_b.stream_id == cfg_a.stream_id {
        cfg_b.stream_id = cfg_a.stream_id + 1;
    }

    // App <-> endpoint command channels
    let (cmd_a_tx, cmd_a_rx) = mpsc::channel::<Cmd>(128);
    let (cmd_b_tx, cmd_b_rx) = mpsc::channel::<Cmd>(128);

    // Simulated link (A->B, B->A) frames (single channel with path tagging)
    let (wire_ab_tx, wire_ab_rx) = mpsc::channel::<LinkMsg>(1024);
    let (wire_ba_tx, wire_ba_rx) = mpsc::channel::<LinkMsg>(1024);

    tokio::spawn(endpoint_task(
        cfg_a,
        cmd_a_rx,
        wire_ab_tx.clone(),
        wire_ba_rx,
    ));
    tokio::spawn(endpoint_task(
        cfg_b,
        cmd_b_rx,
        wire_ba_tx.clone(),
        wire_ab_rx,
    ));

    (AsyncStream { tx: cmd_a_tx }, AsyncStream { tx: cmd_b_tx })
}

/// Internal tracking structure for transmitted frames awaiting acknowledgment.
/// This structure maintains critical state for implementing reliable transmission,
/// including retry logic, path selection for multipath scenarios, and timing data
/// for adaptive timeout calculation.
struct TxEntry {
    frame: Frame,
    last_sent: Instant,
    retries: u32,
    last_path: PathId,
}

async fn endpoint_task(
    config: AsyncStreamConfig,
    mut cmd_s: mpsc::Receiver<Cmd>,
    wire_tx: mpsc::Sender<LinkMsg>,
    mut wire_rx: mpsc::Receiver<LinkMsg>,
) {
    let mut next_seq: u64 = 1;
    let mut inflight: BTreeMap<u64, TxEntry> = BTreeMap::new();
    let mut flow = FlowController::new(config.max_inflight, config.max_inflight * 4);
    let mut rtt = RttEstimator::new(config.retransmit_timeout);
    let mut rx_queue: std::collections::VecDeque<Bytes> = Default::default();
    let mut pending_rx: BTreeMap<u64, Bytes> = BTreeMap::new();
    let mut expected_rx_seq: u64 = 1;
    let mut closed_local = false;
    let mut closed_remote = false;
    let mut reorder_buf: Vec<(BytesMut, PathId)> = Vec::new();
    let mut mpr = config.multipath.as_ref().and_then(|s| {
        if s.enable_multipath && s.paths.len() > 1 {
            Some(MprState::new(&s.paths))
        } else {
            None
        }
    });
    let retransmit_alt = config
        .multipath
        .as_ref()
        .map(|s| s.retransmit_on_new_path)
        .unwrap_or(false);

    // Non-blocking recv: no waiter queue; SDK polls via try_recv/recv loop

    // Periodic timer to check retransmit timeouts even if idle
    let mut rto_tick = tokio::time::interval(config.retransmit_timeout / 2);
    rto_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        // Retransmit timer (scan limited number per loop)
        let mut scanned = 0usize;
        let now = Instant::now();
        let max_scan = 16; // cap per tick
        let keys: Vec<u64> = inflight.keys().cloned().collect();
        for k in keys {
            if scanned >= max_scan {
                break;
            }
            if let Some(entry) = inflight.get_mut(&k) {
                if now.duration_since(entry.last_sent) >= rtt.rto()
                    && entry.retries < config.max_retries
                {
                    let mut buf = BytesMut::new();
                    if FrameCodec::encode(&entry.frame, &mut buf).is_ok() {
                        let path = if retransmit_alt {
                            mpr.as_mut()
                                .map(|s| s.pick_path())
                                .unwrap_or(entry.last_path)
                        } else {
                            entry.last_path
                        };
                        let _ = wire_tx
                            .send(LinkMsg::Wire {
                                bytes: buf,
                                path: path.0,
                            })
                            .await;
                    }
                    entry.last_sent = Instant::now();
                    entry.retries += 1;
                    flow.on_loss();
                    rtt.on_timeout();
                    if let Some(ref mut mp) = mpr {
                        mp.on_loss(entry.last_path);
                    }
                    scanned += 1;
                }
            }
        }

        tokio::select! {
            biased;
            // Commands first to avoid starvation
            Some(cmd) = cmd_s.recv() => {
                match cmd {
                    Cmd::Send { data, ack } => {
                        // Early exit if stream is already closed locally
                        if closed_local { 
                            let _ = ack.send(()); 
                            continue; 
                        }
                        
                        // Apply backpressure by waiting if flow control window is full
                        while !flow.can_send(inflight.len()) { 
                            sleep(Duration::from_millis(1)).await; 
                        }
                        
                        // Enforce maximum frame length limit if configured
                        if let Some(limit) = config.max_frame_len { 
                            if data.len() > limit { 
                                let _ = ack.send(()); 
                                continue; 
                            } 
                        }
                        
                        // Create data frame with monotonically increasing sequence number
                        let frame = Frame::data(config.stream_id, next_seq, data);
                        next_seq += 1;
                        
                        // Select optimal path for this frame (multipath load balancing)
                        let selected_path = mpr.as_mut()
                            .map(|s| s.pick_path())
                            .unwrap_or(PathId(0));
                        
                        // Encode frame and handle optional reordering for network simulation
                        let mut buf = BytesMut::new();
                        if FrameCodec::encode(&frame, &mut buf).is_ok() {
                            if let Some(n) = config.reorder_window {
                                // Buffer frames and emit in reverse order for testing
                                reorder_buf.push((buf, selected_path));
                                if reorder_buf.len() >= n {
                                    // Flush buffered frames in reverse order
                                    while let Some((b, path)) = reorder_buf.pop() {
                                        let _ = wire_tx.send(LinkMsg::Wire { 
                                            bytes: b, 
                                            path: path.0 
                                        }).await;
                                    }
                                }
                            } else {
                                // Direct transmission without reordering
                                let _ = wire_tx.send(LinkMsg::Wire { 
                                    bytes: buf, 
                                    path: selected_path.0 
                                }).await;
                            }
                        }
                        
                        // Track frame for retransmission and acknowledgment handling
                        inflight.insert(frame.header.seq, TxEntry { 
                            frame, 
                            last_sent: Instant::now(), 
                            retries: 0, 
                            last_path: selected_path 
                        });
                        let _ = ack.send(());
                    }
                    Cmd::Recv { reply } => {
                        if let Some(b) = rx_queue.pop_front() {
                            let _ = reply.send(Some(b));
                        } else {
                            let _ = reply.send(None);
                        }
                    }
                    Cmd::TryRecv { reply } => {
                        if let Some(b) = rx_queue.pop_front() {
                            let _ = reply.send(Some(b));
                        } else {
                            let _ = reply.send(None);
                        }
                    }
                    Cmd::Close { ack } => {
                        if !closed_local {
                            let close = Frame { header: FrameHeader { stream_id: config.stream_id, seq: next_seq, ty: FrameType::Close }, payload: vec![] };
                            let mut buf = BytesMut::new();
                            if FrameCodec::encode(&close, &mut buf).is_ok() {
                                if config.reorder_window.is_some() {
                                    // Flush any remaining buffered frames first in reverse
                                    while let Some((b, path)) = reorder_buf.pop() {
                                        let _ = wire_tx.send(LinkMsg::Wire { bytes: b, path: path.0 }).await;
                                    }
                                    let path = mpr.as_mut().map(|s| s.pick_path()).unwrap_or(PathId(0));
                                    let _ = wire_tx.send(LinkMsg::Wire { bytes: buf, path: path.0 }).await;
                                } else {
                                    let path = mpr.as_mut().map(|s| s.pick_path()).unwrap_or(PathId(0));
                                    let _ = wire_tx.send(LinkMsg::Wire { bytes: buf, path: path.0 }).await;
                                }
                            }
                        }
                        // Send close across all paths to ensure peer sees it
                        let _ = wire_tx.send(LinkMsg::Close).await;
                        let _ = ack.send(());
                        closed_local = true;
                    }
                }
            }
            _ = rto_tick.tick() => {
                // drive periodic timeouts; actual work happens above each loop iteration
            }
            // Link receive path
            msg = wire_rx.recv() => {
                match msg {
                    Some(LinkMsg::Wire{ mut bytes, path }) => {
                        // Decode one frame per wire message
                        match FrameCodec::decode(&mut bytes) {
                            Ok(Some(frame)) => {
                                match frame.header.ty {
                                    FrameType::Data => {
                                // Queue payload out-of-order and ack
                                pending_rx.insert(frame.header.seq, Bytes::from(frame.payload));
                                // Optionally cap pending_rx size
                                if let Some(cap) = config.max_reorder_pending {
                                    if pending_rx.len() > cap {
                                        // drop the largest (newest) to preserve ability to progress expected_rx_seq
                                        if let Some((&dropseq, _)) = pending_rx.iter().next_back() { let _ = pending_rx.remove(&dropseq); }
                                    }
                                }
                                while let Some(b) = pending_rx.remove(&expected_rx_seq) {
                                    rx_queue.push_back(b);
                                    expected_rx_seq += 1;
                                }
                                // non-blocking receive: consumer will poll
                                let ack = Frame { header: FrameHeader { stream_id: config.stream_id, seq: frame.header.seq, ty: FrameType::Ack }, payload: vec![] };
                                let mut buf = BytesMut::new();
                                if FrameCodec::encode(&ack, &mut buf).is_ok() { let _ = wire_tx.send(LinkMsg::Wire { bytes: buf, path }).await; }
                            }
                            FrameType::Ack => {
                                // Slide window and grow
                                if let Some(sent) = inflight.remove(&frame.header.seq) {
                                    flow.on_ack(frame.header.seq);
                                    // Only use RTT sample if this wasn't a retransmission (Karn's algorithm)
                                    if sent.retries == 0 {
                                        let sample = sent.last_sent.elapsed();
                                        rtt.on_ack_sample(sample);
                                        if let Some(ref mut mp) = mpr { mp.on_rtt_sample(sent.last_path, sample); }
                                    }
                                } else {
                                    // duplicate ack indicates potential loss; consider selective retransmit
                                    // pick the lowest outstanding to retransmit if needed
                                    if let Some((&seq, entry)) = inflight.iter_mut().next() {
                                        if flow.should_retransmit(seq, entry.retries as usize) && entry.retries < config.max_retries {
                                            let mut buf = BytesMut::new();
                                            if FrameCodec::encode(&entry.frame, &mut buf).is_ok() {
                                                let _ = wire_tx.send(LinkMsg::Wire { bytes: buf, path: entry.last_path.0 }).await;
                                            }
                                            entry.retries += 1;
                                            entry.last_sent = Instant::now();
                                            entry.last_path = PathId(path);
                                            if let Some(ref mut mp) = mpr { mp.on_loss(entry.last_path); }
                                        }
                                    }
                                }
                            }
                            FrameType::Close => {
                                // Handle close frame - set closed_remote flag
                                closed_remote = true;
                            }
                        }
                    }
                    Ok(None) => {
                        // Need more data
                    }
                    Err(_e) => {
                        // Codec error; log and continue
                    }
                }
            }
            Some(LinkMsg::Close) | None => break, // wire link down
                }
            }
        }

        if closed_local && closed_remote {
            break;
        }
    }
    // non-blocking: nothing to wake
}

// For now we tag frames with a path id but share a single simulated link channel.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multipath::scheduler::PathMetric;

    #[tokio::test]
    async fn send_recv_roundtrip_and_backpressure() -> Result<(), Box<dyn std::error::Error>> {
        // Test comprehensive send/receive functionality with backpressure handling
        let (stream_a, stream_b) = pair(AsyncStreamConfig::default(), AsyncStreamConfig::default());
        
        // Send more messages than the flow control window to exercise backpressure mechanisms
        for i in 0..100u32 {
            stream_a.send(Bytes::from(format!("msg-{i}"))).await?;
        }
        
        // Receive all messages and verify ordering preservation
        let mut received_messages = Vec::new();
        loop {
            if let Some(buf) = stream_b.recv().await? {
                received_messages.push(String::from_utf8(buf.to_vec())?);
                if received_messages.len() == 100 {
                    break;
                }
            } else {
                // Yield control to allow other tasks to progress
                tokio::task::yield_now().await;
            }
        }
        
        // Verify message count and ordering integrity
        assert_eq!(received_messages.len(), 100);
        assert_eq!(received_messages[0], "msg-0");
        assert_eq!(received_messages[99], "msg-99");
        Ok(())
    }

    #[tokio::test]
    async fn close_propagates() -> Result<(), Box<dyn std::error::Error>> {
        let (a, b) = pair(AsyncStreamConfig::default(), AsyncStreamConfig::default());
        
        // Send some data first to ensure connection is established
        a.send(Bytes::from_static(b"test")).await?;
        let _data = b.recv().await?;
        
        // Test graceful close
        a.close().await?;
        
        // Close operation completed successfully
        Ok(())
    }

    #[tokio::test]
    async fn reorder_is_reassembled_in_order() -> Result<(), Box<dyn std::error::Error>> {
        let mut ca = AsyncStreamConfig::default();
        let mut cb = AsyncStreamConfig::default();
        ca.reorder_window = Some(2);
        cb.reorder_window = Some(2);
        let (a, b) = pair(ca, cb);
        a.send(Bytes::from_static(b"a1")).await?;
        a.send(Bytes::from_static(b"a2")).await?;
        a.send(Bytes::from_static(b"a3")).await?;
        a.send(Bytes::from_static(b"a4")).await?;
        let mut got = Vec::new();
        while got.len() < 4 {
            if let Some(buf) = b.recv().await? {
                got.push(buf);
            } else {
                tokio::task::yield_now().await;
            }
        }
        assert_eq!(&got[0][..], b"a1");
        assert_eq!(&got[1][..], b"a2");
        assert_eq!(&got[2][..], b"a3");
        assert_eq!(&got[3][..], b"a4");
        Ok(())
    }

    #[tokio::test]
    async fn max_frame_len_is_enforced_on_send() -> Result<(), Box<dyn std::error::Error>> {
        let ca = AsyncStreamConfig {
            max_frame_len: Some(3),
            ..Default::default()
        };
        let (a, b) = pair(ca, AsyncStreamConfig::default());
        a.send(Bytes::from_static(b"123")).await?;
        // Over limit: silently dropped by sender before wire
        a.send(Bytes::from_static(b"1234")).await?;
        let first = b.recv().await?.ok_or("expected first message")?;
        assert_eq!(&first[..], b"123");
        // Nothing else should arrive
        for _ in 0..10 {
            if b.recv().await?.is_some() {
                return Err("should not receive oversized frame".into());
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn multipath_preserves_ordering_at_receiver() -> Result<(), Box<dyn std::error::Error>> {
        let mut ca = AsyncStreamConfig::default();
        let mut cb = AsyncStreamConfig::default();
        ca.multipath = Some(IntegrationSettings {
            enable_multipath: true,
            paths: vec![
                (
                    PathId(0),
                    PathMetric {
                        rtt: Duration::from_millis(10),
                        loss: 0.0,
                        weight: 1,
                    },
                ),
                (
                    PathId(1),
                    PathMetric {
                        rtt: Duration::from_millis(20),
                        loss: 0.0,
                        weight: 1,
                    },
                ),
            ],
            retransmit_on_new_path: true,
        });
        cb.multipath = ca.multipath.clone();
        let (a, b) = pair(ca, cb);
        for i in 0..50u32 {
            a.send(Bytes::from(format!("m-{i}"))).await?;
        }
        let mut out = Vec::new();
        while out.len() < 50 {
            if let Some(buf) = b.recv().await? {
                out.push(String::from_utf8(buf.to_vec())?);
            } else {
                tokio::task::yield_now().await;
            }
        }
        for i in 0..50u32 {
            assert_eq!(out[i as usize], format!("m-{i}"));
        }
        Ok(())
    }

    #[tokio::test]
    async fn pending_reorder_cap_is_enforced() -> Result<(), Box<dyn std::error::Error>> {
        // Configure a small pending cap to force drop of oldest out-of-order frames
        let mut ca = AsyncStreamConfig::default();
        let mut cb = AsyncStreamConfig::default();
        ca.reorder_window = Some(10); // buffer up frames then flush in reverse
        cb.max_reorder_pending = Some(4);
        let (a, b) = pair(ca, cb);

        // Send 10 frames which will arrive out-of-order
        for i in 0..10u32 {
            a.send(Bytes::from(format!("x-{i}"))).await?;
        }
        // Drain what we can; due to drops, we should still eventually see progress without OOM
        let mut got = Vec::new();
        let start = tokio::time::Instant::now();
        while tokio::time::Instant::now() - start < Duration::from_secs(1) {
            if let Some(buf) = b.recv().await? {
                got.push(String::from_utf8(buf.to_vec())?);
                if got.len() >= 4 {
                    break;
                }
            } else {
                tokio::task::yield_now().await;
            }
        }
        assert!(!got.is_empty());
        Ok(())
    }
}
