#![forbid(unsafe_code)]

use tokio::sync::mpsc;
#[cfg(feature = "fec")]
pub use nyx_fec::{TimingObfuscator, TimingConfig, Packet};
use super::Sequencer;
use tracing::instrument;
#[cfg(feature = "hpke")]
use crate::{HpkeRekeyManager, RekeyDecision, seal_for_rekey};
#[cfg(feature = "hpke")]
use nyx_crypto::hpke::{PublicKey};
#[cfg(feature = "hpke")]
use nyx_crypto::noise::SessionKey; // may be needed later for external APIs
#[cfg(feature = "telemetry")]
use nyx_telemetry::{inc_hpke_rekey_initiated, inc_hpke_rekey_applied};
#[cfg(feature = "telemetry")]
use nyx_telemetry::record_stream_send;

// Compatibility implementation used when the `fec` feature is disabled.
// Mirrors the public surface and timing semantics of `nyx_fec::timing` so that
// higher layers behave consistently regardless of the feature flag.
#[cfg(not(feature = "fec"))]
mod fec_compat {
    use super::mpsc;
    use std::f64::consts::PI;
    use tokio::time::{sleep, Duration};

    /// Timing parameters for obfuscation delay.
    #[derive(Clone, Copy, Debug)]
    pub struct TimingConfig {
        /// Mean delay in milliseconds.
        pub mean_ms: f64,
        /// Standard deviation of delay in milliseconds.
        pub sigma_ms: f64,
    }

    impl Default for TimingConfig {
        fn default() -> Self {
            // Keep defaults aligned with `nyx_fec::timing::TimingConfig` for parity.
            Self { mean_ms: 20.0, sigma_ms: 10.0 }
        }
    }

    /// Obfuscated packet wrapper to match FEC timing API.
    #[derive(Clone)]
    pub struct Packet(pub Vec<u8>);

    /// Queue that releases packets after randomized delay.
    pub struct TimingObfuscator {
        in_tx: mpsc::Sender<Packet>,
        out_rx: mpsc::Receiver<Packet>,
    }

    impl TimingObfuscator {
        /// Create a timing obfuscator. When `sigma_ms > 0`, delays are sampled from
        /// a normal distribution N(mean_ms, sigma_ms). The delay is clamped to >= 0ms.
        pub fn new(cfg: TimingConfig) -> Self {
            let (in_tx, mut in_rx) = mpsc::channel::<Packet>(1024);
            let (out_tx, out_rx) = mpsc::channel::<Packet>(1024);

            tokio::spawn(async move {
                while let Some(pkt) = in_rx.recv().await {
                    let delay_ms = sample_non_negative_normal_ms(cfg.mean_ms, cfg.sigma_ms);
                    if delay_ms > 0.0 {
                        sleep(Duration::from_millis(delay_ms as u64)).await;
                    }
                    if out_tx.send(pkt).await.is_err() {
                        break;
                    }
                }
            });

            Self { in_tx, out_rx }
        }

        /// Get a clone of the internal sender so producers can enqueue packets.
        pub fn sender(&self) -> mpsc::Sender<Packet> { self.in_tx.clone() }

        /// Receive next obfuscated packet.
        pub async fn recv(&mut self) -> Option<Packet> { self.out_rx.recv().await }
    }

    /// Sample a non-negative delay in milliseconds from N(mean, sigma).
    /// Uses the Box-Muller transform with `fastrand` as the RNG backend.
    fn sample_non_negative_normal_ms(mean_ms: f64, sigma_ms: f64) -> f64 {
        if sigma_ms <= 0.0 {
            return mean_ms.max(0.0);
        }
        // Box-Muller transform: Z0 ~ N(0,1) from two independent U(0,1].
        // Clamp u1 away from 0 to avoid ln(0).
        let u1 = loop {
            let v = fastrand::f64();
            if v > f64::MIN_POSITIVE { break v; }
        };
        let u2 = fastrand::f64();
        let z0 = (-2.0_f64 * u1.ln()).sqrt() * (2.0 * PI * u2).cos();
        let sample = mean_ms + z0 * sigma_ms;
        sample.max(0.0)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tokio::time::Instant;

        #[tokio::test]
        async fn delay_not_excessive_for_reasonable_params() {
            // This is a soft timing check to ensure the compat layer behaves similarly
            // to the main `nyx_fec` timing implementation.
            let cfg = TimingConfig { mean_ms: 15.0, sigma_ms: 5.0 };
            let obf = TimingObfuscator::new(cfg);
            let start = Instant::now();
            let tx = obf.sender();
            let mut rx = obf;
            // Enqueue a single packet and expect delivery not far beyond ~mean+3*sigma.
            tx.send(Packet(vec![1, 2, 3])).await.ok();
            let _ = rx.recv().await.expect("packet should be delivered");
            let elapsed = start.elapsed().as_millis();
            assert!(elapsed <= 80, "elapsed {}ms too large", elapsed);
        }
    }
}

#[cfg(not(feature = "fec"))]
pub use fec_compat::{TimingObfuscator, TimingConfig, Packet};

/// TxQueue integrates TimingObfuscator and provides outgoing packet stream.
pub struct TxQueue {
    in_tx: mpsc::Sender<Packet>,
    out_rx: mpsc::Receiver<Vec<u8>>, // obfuscated frames for transport
    sequencer: tokio::sync::Mutex<Sequencer>,
    #[cfg(feature = "hpke")]
    rekey_mgr: tokio::sync::Mutex<Option<HpkeRekeyManager>>, // None until enabled
    #[cfg(feature = "hpke")]
    peer_hpke_pk: Option<PublicKey>,
    #[cfg(feature = "hpke")]
    pending_rekey_frames: tokio::sync::Mutex<Vec<Vec<u8>>>, // serialized frames waiting for control channel
}

impl TxQueue {
    pub fn new(cfg: TimingConfig) -> Self {
        let mut obf = TimingObfuscator::new(cfg);

        let in_tx = obf.sender();
        let (out_tx, out_rx) = mpsc::channel::<Vec<u8>>(1024);

        // Task: forward from obf.recv -> out_tx
        tokio::spawn(async move {
            while let Some(pkt) = obf.recv().await {
                let Packet(bytes) = pkt;
                if out_tx.send(bytes).await.is_err() {
                    break;
                }
            }
        });

    Self { in_tx, out_rx, sequencer: tokio::sync::Mutex::new(Sequencer::new()), #[cfg(feature="hpke")] rekey_mgr: tokio::sync::Mutex::new(None), #[cfg(feature="hpke")] peer_hpke_pk: None, #[cfg(feature="hpke")] pending_rekey_frames: tokio::sync::Mutex::new(Vec::new()) }
    }

    #[cfg(feature = "hpke")]
    pub async fn enable_hpke_rekey(&mut self, mgr: HpkeRekeyManager, peer_pk: PublicKey) {
        *self.rekey_mgr.lock().await = Some(mgr);
        self.peer_hpke_pk = Some(peer_pk);
    }

    #[cfg(feature = "hpke")]
    pub async fn hpke_current_key_clone(&self) -> Option<SessionKey> {
        self.rekey_mgr.lock().await.as_ref().map(|m| m.current_key().clone())
    }

    #[cfg(feature = "hpke")]
    pub async fn drain_rekey_frames(&self) -> Vec<Vec<u8>> {
        let mut guard = self.pending_rekey_frames.lock().await;
        let out = guard.clone();
        guard.clear();
        out
    }

    #[cfg(feature = "hpke")]
    pub async fn send_all_rekey_frames_via<F>(&self, mut sender: F) where F: FnMut(&[u8]) -> bool {
        // Drain then attempt immediate send through provided closure (e.g., control channel writer)
        let frames = self.drain_rekey_frames().await;
        for f in frames { let _ = sender(&f); }
    }

    #[cfg(feature = "hpke")]
    pub async fn flush_rekey_frames<F>(&self, mut sender: F) where F: FnMut(&[u8]) -> bool {
        let mut guard = self.pending_rekey_frames.lock().await;
        let mut retained = Vec::new();
        for frame in guard.iter() {
            if !sender(frame) { retained.push(frame.clone()); }
        }
        *guard = retained; // keep unsent
    }

    #[cfg(feature = "hpke")]
    pub async fn flush_rekey_frames_async<F, Fut>(&self, mut sender: F) where F: FnMut(Vec<u8>) -> Fut, Fut: std::future::Future<Output=bool> {
        let mut guard = self.pending_rekey_frames.lock().await;
        let mut retained = Vec::new();
        // drain in FIFO order
        for frame in guard.drain(..) {
            if !sender(frame.clone()).await { retained.push(frame); }
        }
        *guard = retained;
    }

    /// Send frame without specific PathID.  Emits OTLP span `nyx.stream.send`.
    #[instrument(name = "nyx.stream.send", skip_all, fields(path_id = -1i8, cid = "unknown"))]
    pub async fn send(&self, bytes: Vec<u8>) {
        let _ = self.in_tx.send(Packet(bytes)).await;
    #[cfg(feature = "telemetry")]
    record_stream_send(255, "unknown"); // 255 sentinel for no specific path
    }

    /// Send bytes tagged with PathID, returning assigned sequence number.
    /// Emits OTLP span `nyx.stream.send` with `path_id` attribute.
    #[instrument(name = "nyx.stream.send", skip_all, fields(path_id = path_id, cid = "unknown"))]
    pub async fn send_with_path(&self, path_id: u8, bytes: Vec<u8>) -> u64 {
        let mut seq = self.sequencer.lock().await;
        let s = seq.next(path_id);
        // prepend seq (8 bytes LE) for now; protocol integration later.
        let mut buf = Vec::with_capacity(8 + bytes.len());
        buf.extend_from_slice(&s.to_le_bytes());
        buf.extend_from_slice(&bytes);
        let _ = self.in_tx.send(Packet(buf)).await;
    #[cfg(feature = "telemetry")]
    record_stream_send(path_id, "unknown");
    #[cfg(feature = "hpke")]
    {
        // Evaluate rekey policy after each packet send.
        if let Some(mgr) = self.rekey_mgr.lock().await.as_mut() {
            match mgr.on_packet_sent() {
                RekeyDecision::NoAction => {},
                RekeyDecision::Initiate => {
                    if let Some(peer_pk) = &self.peer_hpke_pk {
                        if let Ok((frame, new_key)) = seal_for_rekey(peer_pk, b"nyx-hpke-rekey") {
                            // Install new key locally
                            mgr.install_new_key(new_key);
                            #[cfg(feature="telemetry")]
                            {
                                inc_hpke_rekey_initiated();
                                inc_hpke_rekey_applied();
                            }
                            // Serialize and stash frame for later control channel transmission
                            let bytes = crate::build_rekey_frame(&frame);
                            self.pending_rekey_frames.lock().await.push(bytes);
                            // NOTE: Future: if a control channel handle is registered, attempt immediate send here.
                        } else {
                            #[cfg(feature="telemetry")]
                            {
                                nyx_telemetry::inc_hpke_rekey_failure();
                                nyx_telemetry::inc_hpke_rekey_failure_reason("generate");
                            }
                        }
                    }
                }
            }
        }
    }
        s
    }

    pub async fn recv(&mut self) -> Option<Vec<u8>> {
        self.out_rx.recv().await
    }

    /// Provide a sender clone for external producers.
    pub fn clone_sender(&self) -> mpsc::Sender<Packet> {
        self.in_tx.clone()
    }
}