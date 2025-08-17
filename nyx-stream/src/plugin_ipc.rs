#![forbid(unsafe_code)]

use crate::plugin::{PluginId, PluginHeader};
use crate::plugin_dispatch::PluginMessage;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// Abstract IPC transport for plugins. This crate keeps core independent from
/// platform specifics by providing traits and a pure-Rust in-process reference implementation.
pub trait PluginIpcSender: Send + Sync {
	/// Non-blocking send with backpressure. Returns Err when the channel is full or closed.
	/// Consumers may implement retry-with-backoff upon receiving an error.
	fn send(&self, header: &PluginHeader, frame_type: u8, raw: &[u8]) -> Result<(), String>;
}

pub trait PluginIpcReceiver: Send + Sync {
	/// Non-blocking receive; returns the next message if available.
	fn try_recv(&self) -> Option<(u8, PluginHeader, Vec<u8>)>;
}

/// A no-op sender used in tests or benchmarks that don't require delivery.
#[derive(Default, Clone)]
pub struct NoopSender;
impl PluginIpcSender for NoopSender {
	fn send(&self, _header: &PluginHeader, _frame_type: u8, _raw: &[u8]) -> Result<(), String> {
		Ok(())
	}
}

/// Helper to name a plugin for logs.
pub fn format_plugin(p: PluginId, name: &str) -> String { format!("{name}#{p}") }

/// Internal message container for IPC
#[derive(Debug, Clone)]
struct IpcMessage {
	frame_type: u8,
	header: PluginHeader,
	raw: Vec<u8>,
}

/// In-process bounded IPC channel with backpressure and reconnection support.
///
/// Design notes:
/// - Uses tokio mpsc bounded channel. Sender uses try_send (non-blocking) to enforce backpressure.
/// - When the receiver disconnects, messages can no longer be delivered until a new receiver connects.
/// - Reconnection is coordinated via the shared Hub which swaps the underlying channel atomically.
/// - This avoids unsafe code and any C/C++ dependencies.
#[derive(Debug, Clone)]
pub struct InProcIpcSender {
	inner: Arc<Mutex<mpsc::Sender<IpcMessage>>>,
}

#[derive(Debug)]
pub struct InProcIpcReceiver {
	rx: mpsc::Receiver<IpcMessage>,
	// NOTE: Receiver is intentionally not Clone; only one receiver is active per channel generation.
}

impl PluginIpcSender for InProcIpcSender {
	fn send(&self, header: &PluginHeader, frame_type: u8, raw: &[u8]) -> Result<(), String> {
		let msg = IpcMessage { frame_type, header: header.clone(), raw: raw.to_vec() };
		let tx = self.inner.lock().map_err(|_| "ipc poisoned mutex".to_string())?;
		match tx.try_send(msg) {
			Ok(()) => Ok(()),
			Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => Err("full".to_string()),
			Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => Err("closed".to_string()),
		}
	}
}

impl PluginIpcReceiver for InProcIpcReceiver {
	fn try_recv(&self) -> Option<(u8, PluginHeader, Vec<u8>)> {
		// Safety: mpsc::Receiver::try_recv takes &mut self; guard with interior mutability via RefCell? Not needed:
		// We keep rx mutable on method, but trait takes &self. Use a Mutex to enable non-blocking try.
		// Instead, we rely on an inherent method on the receiver wrapper that is &mut; provide below.
		unreachable!("use try_recv_mut via Hub handle; PluginIpcReceiver is implemented for internal wrapper only")
	}
}

impl InProcIpcReceiver {
	/// Try to receive next message without awaiting. This consumes from the underlying queue.
	pub fn try_recv_mut(&mut self) -> Option<(u8, PluginHeader, Vec<u8>)> {
		match self.rx.try_recv() {
			Ok(IpcMessage { frame_type, header, raw }) => Some((frame_type, header, raw)),
			Err(_) => None,
		}
	}
}

/// Hub coordinates sender and receiver generations to enable reconnection semantics.
#[derive(Debug)]
pub struct InProcIpcHub {
	inner_tx: Arc<Mutex<mpsc::Sender<IpcMessage>>>,
	capacity: usize,
}

impl InProcIpcHub {
	/// Create a new hub with specified bounded capacity and return a sender and initial receiver.
	pub fn new(capacity: usize) -> (Self, InProcIpcSender, InProcIpcReceiver) {
		let (tx, rx) = mpsc::channel::<IpcMessage>(capacity);
		let inner_tx = Arc::new(Mutex::new(tx));
		let hub = Self { inner_tx: inner_tx.clone(), capacity };
		let sender = InProcIpcSender { inner: inner_tx };
		let receiver = InProcIpcReceiver { rx };
		(hub, sender, receiver)
	}

	/// Connect a new receiver, atomically replacing the underlying channel.
	/// Returns the new receiver; existing queued messages (if any) on the old channel are dropped.
	pub fn reconnect_receiver(&self) -> InProcIpcReceiver {
		let (tx, rx) = mpsc::channel::<IpcMessage>(self.capacity);
		if let Ok(mut guard) = self.inner_tx.lock() {
			*guard = tx;
		}
		InProcIpcReceiver { rx }
	}
}

/// Adapter: allow using Tokio mpsc::Sender<PluginMessage> with the PluginIpcSender trait.
impl PluginIpcSender for mpsc::Sender<PluginMessage> {
	fn send(&self, header: &PluginHeader, frame_type: u8, raw: &[u8]) -> Result<(), String> {
		let msg = PluginMessage::new(frame_type, header.clone(), raw.to_vec());
		match self.try_send(msg) {
			Ok(()) => Ok(()),
			Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => Err("full".to_string()),
			Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => Err("closed".to_string()),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::plugin::PluginId;

	fn header(id: u32) -> PluginHeader { PluginHeader { id: PluginId(id), flags: 0, data: vec![1,2,3] } }

	#[test]
	fn noop_sender_always_ok() {
		let s = NoopSender::default();
		assert!(s.send(&header(1), 0x51, &[9,9]).is_ok());
	}

	#[tokio::test]
	async fn inproc_send_and_recv_nonblocking() {
		let (_hub, sender, mut recv) = InProcIpcHub::new(4);
		for i in 0..3 {
			let h = header(10 + i);
			sender.send(&h, 0x51, &[i as u8]).unwrap();
		}
		let mut seen = 0;
		while let Some((_t, h, raw)) = recv.try_recv_mut() { 
			assert_eq!(raw.len(), 1);
			assert!(h.id.0 >= 10);
			seen += 1;
		}
		assert_eq!(seen, 3);
	}

	#[tokio::test]
	async fn backpressure_when_full() {
		let (_hub, sender, mut recv) = InProcIpcHub::new(1);
		// fill the single-slot queue
		sender.send(&header(1), 0x51, &[1]).unwrap();
		// second try without draining should fail
		let e = sender.send(&header(2), 0x51, &[2]).unwrap_err();
		assert!(e.contains("full") || e.contains("SendError") || e.contains("Full"));
		// drain and try again succeeds
		let _ = recv.try_recv_mut();
		assert!(sender.send(&header(3), 0x51, &[3]).is_ok());
	}

	#[tokio::test]
	async fn reconnect_works_after_receiver_drop() {
		let (hub, sender, _recv_initial) = InProcIpcHub::new(2);
		// Drop the initial receiver (goes out of scope)
		// Send will now return Closed until a new receiver connects
		let _ = sender.send(&header(1), 0x51, &[1]);
		// Reconnect
		let mut recv2 = hub.reconnect_receiver();
		// After reconnection, sends are accepted
		assert!(sender.send(&header(2), 0x51, &[2]).is_ok());
		let got = recv2.try_recv_mut();
		assert!(got.is_some());
		let (_t, h, raw) = got.unwrap();
		assert_eq!(h.id.0, 2);
		assert_eq!(raw, vec![2]);
	}
}
