#![forbid(unsafe_code)]

use crate::{errors::{Error, Result}, frame::{Frame, FrameHeader, FrameType}, frame_codec::FrameCodec, flow_controller::FlowController, congestion::RttEstimator};
use crate::multipath::{integration::IntegrationSettings, mpr::{MprState}, scheduler::{PathId}};
use bytes::{Bytes, BytesMut};
use std::{collections::BTreeMap, time::Duration};
use tokio::{sync::{mpsc, oneshot}, time::{Instant, sleep}};

#[derive(Debug, Clone)]
pub struct AsyncStreamConfig {
	pub stream_id: u32,
	pub max_inflight: usize,
	pub retransmit_timeout: Duration,
	pub max_retries: u32,
	/// Optional: deterministic reordering on the wire for testing.
	/// If set to Some(N), the sender buffers N frames and emits them in reverse order.
	pub reorder_window: Option<usize>,
	/// Optional max payload size (bytes). If Some(n), payload larger than n is rejected on send.
	pub max_frame_len: Option<usize>,
	/// Optional multipath settings. If None or disabled, single path is used.
	pub multipath: Option<IntegrationSettings>,
}

impl Default for AsyncStreamConfig {
	fn default() -> Self {
	Self { stream_id: 1, max_inflight: 32, retransmit_timeout: Duration::from_millis(250), max_retries: 8, reorder_window: None, max_frame_len: None, multipath: None }
	}
}

#[derive(Debug)]
enum Cmd {
	Send { data: Bytes, ack: oneshot::Sender<()> },
	Recv { reply: oneshot::Sender<Option<Bytes>> },
	Close { ack: oneshot::Sender<()> },
}

#[derive(Debug)]
enum LinkMsg { Wire { bytes: BytesMut, path: u8 }, Close }

#[derive(Debug, Clone)]
pub struct AsyncStream {
	tx: mpsc::Sender<Cmd>,
}

impl AsyncStream {
	pub async fn send(&self, data: Bytes) -> Result<()> {
		let (tx, rx) = oneshot::channel();
		self.tx.send(Cmd::Send { data, ack: tx }).await.map_err(|_| Error::ChannelClosed)?;
		rx.await.map_err(|_| Error::ChannelClosed)?;
		Ok(())
	}

	pub async fn recv(&self) -> Result<Option<Bytes>> {
		let (tx, rx) = oneshot::channel();
		self.tx.send(Cmd::Recv { reply: tx }).await.map_err(|_| Error::ChannelClosed)?;
		Ok(rx.await.map_err(|_| Error::ChannelClosed)?)
	}

	pub async fn close(&self) -> Result<()> {
		let (tx, rx) = oneshot::channel();
		self.tx.send(Cmd::Close { ack: tx }).await.map_err(|_| Error::ChannelClosed)?;
		let _ = rx.await;
		Ok(())
	}
}

pub fn pair(cfg_a: AsyncStreamConfig, mut cfg_b: AsyncStreamConfig) -> (AsyncStream, AsyncStream) {
	// Ensure distinct stream ids (A->B uses A.stream_id, B->A uses B.stream_id)
	if cfg_b.stream_id == cfg_a.stream_id { cfg_b.stream_id = cfg_a.stream_id + 1; }

	// App <-> endpoint command channels
	let (cmd_a_tx, cmd_a_rx) = mpsc::channel::<Cmd>(128);
	let (cmd_b_tx, cmd_b_rx) = mpsc::channel::<Cmd>(128);

	// Simulated link (A->B, B->A) frames (single channel with path tagging)
	let (wire_ab_tx, wire_ab_rx) = mpsc::channel::<LinkMsg>(1024);
	let (wire_ba_tx, wire_ba_rx) = mpsc::channel::<LinkMsg>(1024);

	tokio::spawn(endpoint_task(cfg_a, cmd_a_rx, wire_ab_tx.clone(), wire_ba_rx));
	tokio::spawn(endpoint_task(cfg_b, cmd_b_rx, wire_ba_tx.clone(), wire_ab_rx));

	(AsyncStream { tx: cmd_a_tx }, AsyncStream { tx: cmd_b_tx })
}

struct TxEntry {
	frame: Frame,
	last_sent: Instant,
	retries: u32,
	last_path: PathId,
}

async fn endpoint_task(
	cfg: AsyncStreamConfig,
	mut cmds: mpsc::Receiver<Cmd>,
	wire_tx: mpsc::Sender<LinkMsg>,
	mut wire_rx: mpsc::Receiver<LinkMsg>,
) {
	let mut next_seq: u64 = 1;
	let mut inflight: BTreeMap<u64, TxEntry> = BTreeMap::new();
	let mut flow = FlowController::new(cfg.max_inflight, cfg.max_inflight * 4);
	let mut rtt = RttEstimator::new(cfg.retransmit_timeout);
	let mut rx_queue: std::collections::VecDeque<Bytes> = Default::default();
	let mut pending_rx: BTreeMap<u64, Bytes> = BTreeMap::new();
	let mut expected_rx_seq: u64 = 1;
	let closed_local = false;
	let mut closed_remote = false;
	let mut reorder_buf: Vec<(BytesMut, PathId)> = Vec::new();
	let mut mpr = cfg.multipath.as_ref().and_then(|s| if s.enable_multipath && s.paths.len() > 1 { Some(MprState::new(&s.paths)) } else { None });
	let retransmit_alt = cfg.multipath.as_ref().map(|s| s.retransmit_on_new_path).unwrap_or(false);

	loop {
		// Retransmit timer
	if let Some(entry) = inflight.values_mut().next() {
	    if entry.last_sent.elapsed() >= rtt.rto() && entry.retries < cfg.max_retries {
				let mut buf = BytesMut::new();
				if FrameCodec::encode(&entry.frame, &mut buf).is_ok() {
		    // choose path for retransmit
					let path = if retransmit_alt { mpr.as_mut().map(|s| s.pick_path()).unwrap_or(entry.last_path) } else { entry.last_path };
					let _ = wire_tx.send(LinkMsg::Wire { bytes: buf, path: path.0 }).await;
				}
				entry.last_sent = Instant::now();
				entry.retries += 1;
				// notify flow controller of possible loss to shrink window
				flow.on_loss();
				rtt.on_timeout();
		if let Some(ref mut mp) = mpr { mp.on_loss(entry.last_path); }
			}
		}

		tokio::select! {
			biased;
			// Commands first to avoid starvation
			Some(cmd) = cmds.recv() => {
				match cmd {
		    Cmd::Send { data, ack } => {
						if closed_local { let _ = ack.send(()); continue; }
						while !flow.can_send(inflight.len()) { sleep(Duration::from_millis(1)).await; }
						if let Some(limit) = cfg.max_frame_len { if data.len() > limit { let _ = ack.send(()); continue; } }
						let frame = Frame::data(cfg.stream_id, next_seq, data);
						next_seq += 1;
						// Decide path for this frame now
						let selected_path = mpr.as_mut().map(|s| s.pick_path()).unwrap_or(PathId(0));
						// Encode and send (or buffer) over the simulated wire
						let mut buf = BytesMut::new();
						if FrameCodec::encode(&frame, &mut buf).is_ok() {
							if let Some(n) = cfg.reorder_window {
								reorder_buf.push((buf, selected_path));
								if reorder_buf.len() >= n {
									// Emit in reverse order
									while let Some((b, path)) = reorder_buf.pop() {
										let _ = wire_tx.send(LinkMsg::Wire { bytes: b, path: path.0 }).await;
									}
								}
							} else {
								let _ = wire_tx.send(LinkMsg::Wire { bytes: buf, path: selected_path.0 }).await;
							}
						}
						inflight.insert(frame.header.seq, TxEntry { frame, last_sent: Instant::now(), retries: 0, last_path: selected_path });
						let _ = ack.send(());
					}
					Cmd::Recv { reply } => {
						if let Some(b) = rx_queue.pop_front() { let _ = reply.send(Some(b)); }
						else if closed_remote { let _ = reply.send(None); }
						else { sleep(Duration::from_millis(1)).await; if let Some(b) = rx_queue.pop_front() { let _ = reply.send(Some(b)); } else { let _ = reply.send(None); } }
					}
					Cmd::Close { ack } => {
						if !closed_local {
							let close = Frame { header: FrameHeader { stream_id: cfg.stream_id, seq: next_seq, ty: FrameType::Close }, payload: vec![] };
							let mut buf = BytesMut::new();
							if FrameCodec::encode(&close, &mut buf).is_ok() {
								if let Some(_) = cfg.reorder_window {
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
						break;
					}
				}
			}
			// Link receive path
			msg = wire_rx.recv() => {
				match msg {
					Some(LinkMsg::Wire{ mut bytes, path }) => {
						// Decode one frame per wire message
						match FrameCodec::decode(&mut bytes) {
							Ok(Some(frame)) => match frame.header.ty {
							FrameType::Data => {
								// Queue payload out-of-order and ACK
								pending_rx.insert(frame.header.seq, Bytes::from(frame.payload));
								while let Some(b) = pending_rx.remove(&expected_rx_seq) {
									rx_queue.push_back(b);
									expected_rx_seq += 1;
								}
								let ack = Frame { header: FrameHeader { stream_id: cfg.stream_id, seq: frame.header.seq, ty: FrameType::Ack }, payload: vec![] };
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
										if flow.should_retransmit(seq, entry.retries) && entry.retries < cfg.max_retries {
											let mut buf = BytesMut::new();
											if FrameCodec::encode(&entry.frame, &mut buf).is_ok() {
												let _ = wire_tx.send(LinkMsg::Wire { bytes: buf, path: entry.last_path.0 }).await;
											}
											entry.retries += 1;
											if let Some(ref mut mp) = mpr { mp.on_loss(entry.last_path); }
										}
									}
								}
							}
							FrameType::Close => {
								closed_remote = true;
							}
							},
							Ok(None) => { /* incomplete frame shouldn't happen in this simulation */ }
							Err(_) => { closed_remote = true; }
						}
					}
					Some(LinkMsg::Close) | None => { closed_remote = true; }
				}
			}
		}

		if closed_local && closed_remote { break; }
	}
}

// For now we tag frames with a path id but share a single simulated link channel.

#[cfg(test)]
mod tests {
	use super::*;
	use crate::multipath::scheduler::{PathMetric};

	#[tokio::test]
	async fn send_recv_roundtrip_and_backpressure() {
		let (a, b) = pair(AsyncStreamConfig::default(), AsyncStreamConfig::default());
		// Fill more than window to exercise backpressure
		for i in 0..100u32 {
			a.send(Bytes::from(format!("msg-{i}"))).await.unwrap();
		}
		// Drain on the other side
		let mut got = Vec::new();
		loop {
			if let Some(buf) = b.recv().await.unwrap() { got.push(String::from_utf8(buf.to_vec()).unwrap()); if got.len() == 100 { break; } } else { tokio::task::yield_now().await; }
		}
		assert_eq!(got.len(), 100);
		assert_eq!(got[0], "msg-0");
		assert_eq!(got[99], "msg-99");
	}

	#[tokio::test]
	async fn close_propagates() {
		let (a, b) = pair(AsyncStreamConfig::default(), AsyncStreamConfig::default());
		a.close().await.unwrap();
		// Peer should observe None eventually
		let mut saw_none = false;
		for _ in 0..100 {
			if let Some(_) = b.recv().await.unwrap() { continue; } else { saw_none = true; break; }
		}
		assert!(saw_none);
	}

	#[tokio::test]
	async fn reorder_is_reassembled_in_order() {
		let mut ca = AsyncStreamConfig::default();
		let mut cb = AsyncStreamConfig::default();
		ca.reorder_window = Some(2);
		cb.reorder_window = Some(2);
		let (a, b) = pair(ca, cb);
		a.send(Bytes::from_static(b"a1")).await.unwrap();
		a.send(Bytes::from_static(b"a2")).await.unwrap();
		a.send(Bytes::from_static(b"a3")).await.unwrap();
		a.send(Bytes::from_static(b"a4")).await.unwrap();
		let mut got = Vec::new();
		while got.len() < 4 {
			if let Some(buf) = b.recv().await.unwrap() { got.push(buf); } else { tokio::task::yield_now().await; }
		}
		assert_eq!(&got[0][..], b"a1");
		assert_eq!(&got[1][..], b"a2");
		assert_eq!(&got[2][..], b"a3");
		assert_eq!(&got[3][..], b"a4");
	}

	#[tokio::test]
	async fn max_frame_len_is_enforced_on_send() {
		let mut ca = AsyncStreamConfig::default();
		ca.max_frame_len = Some(3);
		let (a, b) = pair(ca, AsyncStreamConfig::default());
		a.send(Bytes::from_static(b"123")).await.unwrap();
		// Over limit: silently dropped by sender before wire
		a.send(Bytes::from_static(b"1234")).await.unwrap();
		let first = b.recv().await.unwrap().unwrap();
		assert_eq!(&first[..], b"123");
		// Nothing else should arrive
		for _ in 0..10 {
			if let Some(_) = b.recv().await.unwrap() { panic!("should not receive oversized frame"); }
		}
	}

	#[tokio::test]
	async fn multipath_preserves_ordering_at_receiver() {
		let mut ca = AsyncStreamConfig::default();
		let mut cb = AsyncStreamConfig::default();
		ca.multipath = Some(IntegrationSettings{ enable_multipath: true, paths: vec![(PathId(0), PathMetric{ rtt: Duration::from_millis(10), loss: 0.0, weight: 1 }), (PathId(1), PathMetric{ rtt: Duration::from_millis(20), loss: 0.0, weight: 1 })], retransmit_on_new_path: true });
		cb.multipath = ca.multipath.clone();
		let (a, b) = pair(ca, cb);
		for i in 0..50u32 { a.send(Bytes::from(format!("m-{i}"))).await.unwrap(); }
		let mut out = Vec::new();
		while out.len() < 50 {
			if let Some(buf) = b.recv().await.unwrap() { out.push(String::from_utf8(buf.to_vec()).unwrap()); } else { tokio::task::yield_now().await; }
		}
		for i in 0..50u32 { assert_eq!(out[i as usize], format!("m-{i}")); }
	}
}
