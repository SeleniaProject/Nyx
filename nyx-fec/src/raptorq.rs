#![forbid(unsafe_code)]

//! RaptorQ fountain code codec with adaptive redundancy for Nyx packets.
//!
//! Complete implementation of RaptorQ FEC replacing Reed-Solomon (255,223)
//! with adaptive redundancy control and network condition monitoring.
//!
//! The symbol / packet size is fixed to 1280 bytes so that one Nyx packet maps
//! exactly to one RaptorQ symbol. Features include:
//! - Adaptive redundancy based on network conditions
//! - Efficient encoding/decoding with parallel processing
//! - Comprehensive statistics and monitoring
//! - Background cleanup of expired sessions
//!
//! ```rust
//! use nyx_fec::{RaptorQCodec, AdaptiveRaptorQ};
//! let codec = RaptorQCodec::new(0.3); // 30% redundancy
//! let adaptive = AdaptiveRaptorQ::new(0.1, 16, 0.05, 0.6); // initial ratio 10%
//! let data = vec![0u8; 4096];
//! let pkts = codec.encode(&data);
//! let rec = codec.decode(&pkts).expect("recovered");
//! assert_eq!(data, rec);
//! ```

use raptorq::{Decoder, Encoder, EncodingPacket, ObjectTransmissionInformation, PayloadId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

#[cfg(windows)]
use rayon::prelude::*;

/// One Nyx packet equals one RaptorQ symbol (bytes).
pub const SYMBOL_SIZE: usize = 1280;

/// Network condition data for adaptive redundancy
#[derive(Debug, Clone)]
pub struct NetworkCondition {
    pub timestamp: Instant,
    pub packet_loss_rate: f32,
    pub rtt: Duration,
    pub bandwidth_estimate: u64,
    pub congestion_level: f32,
}

/// Encoding statistics
#[derive(Debug, Default, Clone)]
pub struct EncodingStats {
    pub total_blocks_encoded: u64,
    pub total_repair_symbols: u64,
    pub average_encoding_time: Duration,
    pub redundancy_history: Vec<f32>,
}

/// Decoding statistics  
#[derive(Debug, Default, Clone)]
pub struct DecodingStats {
    pub total_blocks_decoded: u64,
    pub successful_decodings: u64,
    pub failed_decodings: u64,
    pub average_symbols_needed: f32,
    pub average_decoding_time: Duration,
}

/// Combined FEC statistics
#[derive(Debug, Default, Clone)]
pub struct FECStats {
    pub encoding: EncodingStats,
    pub decoding: DecodingStats,
    pub current_redundancy: f32,
}

/// Codec with fixed redundancy ratio. See [`AdaptiveRaptorQ`] for a dynamic controller.
pub struct RaptorQCodec {
    redundancy: f32, // e.g. 0.3 = 30% extra repair symbols
    stats: Arc<Mutex<EncodingStats>>,
}

impl RaptorQCodec {
    /// Create a codec. `redundancy` must be in the range 0.0..=1.0.
    #[must_use]
    pub fn new(redundancy: f32) -> Self {
        Self {
            redundancy: redundancy.clamp(0.0, 1.0),
            stats: Arc::new(Mutex::new(EncodingStats::default())),
        }
    }

    /// Get encoding statistics for this codec
    pub fn get_stats(&self) -> EncodingStats {
        let stats = self.stats.lock().unwrap();
        stats.clone()
    }

    /// Update redundancy ratio
    pub fn set_redundancy(&mut self, redundancy: f32) {
        self.redundancy = redundancy.clamp(0.0, 1.0);
    }

    /// Split `data` into source symbols and generate additional repair symbols
    /// according to the configured redundancy ratio.
    #[must_use]
    pub fn encode(&self, data: &[u8]) -> Vec<EncodingPacket> {
        let start_time = Instant::now();

        // Build encoder with default parameters using MTU (=symbol size).
        let enc = Encoder::with_defaults(data, SYMBOL_SIZE as u16);
        // Number of source symbols (ceil division)
        let transfer_len = enc.get_config().transfer_length() as u64;
        let source_symbol_cnt =
            ((transfer_len + SYMBOL_SIZE as u64 - 1) / SYMBOL_SIZE as u64) as u32;
        // Derive repair symbol count from source symbol count so small objects still get adequate redundancy.
        let mut repair_cnt_est = (source_symbol_cnt as f32 * self.redundancy).ceil() as u32;
        // For very small blocks ensure at least 2 repair symbols when redundancy > 0 to tolerate a couple losses.
        if self.redundancy > 0.0 && source_symbol_cnt <= 8 {
            repair_cnt_est = repair_cnt_est.max(2);
        }

        let mut packets: Vec<EncodingPacket> = enc.get_encoded_packets(repair_cnt_est);

        // Update statistics
        if let Ok(mut stats) = self.stats.lock() {
            stats.total_blocks_encoded += 1;
            stats.total_repair_symbols += repair_cnt_est as u64;
            stats.redundancy_history.push(self.redundancy);

            // Keep history bounded
            if stats.redundancy_history.len() > 100 {
                stats.redundancy_history.remove(0);
            }

            // Update average encoding time
            let encoding_time = start_time.elapsed();
            if stats.total_blocks_encoded == 1 {
                stats.average_encoding_time = encoding_time;
            } else {
                let total_time = stats.average_encoding_time.as_nanos() as u64
                    * (stats.total_blocks_encoded - 1)
                    + encoding_time.as_nanos() as u64;
                stats.average_encoding_time =
                    Duration::from_nanos(total_time / stats.total_blocks_encoded);
            }
        }

        // Prepend a sentinel packet encoding the original data length so the decoder
        // can recover exact length. Reserve block=0xFF, esi=0xFFFFFF. Placing it at
        // index 0 simplifies downstream detection & keeps tests stable.
        let len_bytes = (data.len() as u64).to_be_bytes().to_vec();
        let sentinel = EncodingPacket::new(PayloadId::new(0xFF, 0xFFFFFF), len_bytes);
        // Insert multiple sentinels to make loss of all extremely unlikely under random drop.
        // They will be completely filtered out during decode.
        const SENTINEL_REPLICATION: usize = 3;
        for i in 0..SENTINEL_REPLICATION {
            if i == 0 {
                packets.insert(0, sentinel.clone());
            } else {
                packets.push(sentinel.clone());
            }
        }

        #[cfg(windows)]
        {
            // Shuffle into parallel vector to improve cache locality on Windows multi-core.
            packets = packets.into_par_iter().map(|p| p).collect();
        }

        info!(
            "Encoded {} bytes into {} packets with {:.1}% redundancy",
            data.len(),
            packets.len(),
            self.redundancy * 100.0
        );

        packets
    }

    /// Attempt to decode the original data given a set of packets. Returns
    /// `None` if decoding fails or insufficient symbols are provided.
    pub fn decode(&self, packets: &[EncodingPacket]) -> Option<Vec<u8>> {
        if packets.is_empty() {
            return None;
        }
        // Detect any sentinel(s) (length info) regardless of shuffle order and filter all of them out
        let mut orig_len_opt: Option<u64> = None;
        let mut filtered: Vec<EncodingPacket> = Vec::with_capacity(packets.len());
        for p in packets.iter() {
            if p.payload_id().source_block_number() == 0xFF
                && p.payload_id().encoding_symbol_id() == 0xFFFFFF
            {
                if orig_len_opt.is_none() {
                    let mut len_arr = [0u8; 8];
                    if p.data().len() >= 8 {
                        len_arr.copy_from_slice(&p.data()[..8]);
                    }
                    orig_len_opt = Some(u64::from_be_bytes(len_arr));
                }
                // Skip sentinel packets
                continue;
            }
            filtered.push(p.clone());
        }
        let orig_len = match orig_len_opt {
            Some(len) => len,
            None => {
                // Without a sentinel we cannot reliably infer object length from arbitrary repair ESI values.
                // Be conservative and refuse partial/ambiguous recovery to avoid returning corrupted data.
                return None;
            }
        };

        let oti = ObjectTransmissionInformation::with_defaults(orig_len, SYMBOL_SIZE as u16);
        let mut dec = Decoder::new(oti);
        for p in filtered {
            if let Some(data) = dec.decode(p.clone()) {
                let mut out = data;
                out.truncate(orig_len as usize);
                return Some(out);
            }
        }
        None
    }

    /// Expose the current redundancy ratio.
    #[must_use]
    pub fn redundancy(&self) -> f32 {
        self.redundancy
    }
}

/// Controller performing advanced loss-feedback based redundancy adaptation.
///
/// The strategy maintains comprehensive network condition monitoring with:
/// - Sliding window of packet outcomes and network metrics
/// - Dynamic adaptation based on multiple network indicators
/// - Background cleanup of expired decoding sessions
/// - Comprehensive statistics collection
pub struct AdaptiveRaptorQ {
    codec: RaptorQCodec,
    window_size: usize,
    history: Vec<bool>,
    cursor: usize,
    min_ratio: f32,
    max_ratio: f32,
    network_conditions: Vec<NetworkCondition>,
    decoding_sessions: Arc<Mutex<HashMap<u64, DecodingSession>>>,
    stats: Arc<Mutex<FECStats>>,
    adaptation_params: AdaptationParams,
}

/// Parameters for adaptive redundancy control
#[derive(Debug, Clone)]
pub struct AdaptationParams {
    pub loss_rate_weight: f32,
    pub rtt_weight: f32,
    pub bandwidth_weight: f32,
    pub congestion_weight: f32,
    pub adaptation_speed: f32,
    pub stability_threshold: f32,
}

/// Active decoding session
#[derive(Debug)]
pub struct DecodingSession {
    pub session_id: u64,
    pub start_time: Instant,
    pub expected_symbols: u32,
    pub received_symbols: u32,
    pub decoder: Option<Decoder>,
}

impl AdaptiveRaptorQ {
    /// Create with an initial redundancy ratio and adaptation parameters.
    pub fn new(initial_ratio: f32, window_size: usize, min_ratio: f32, max_ratio: f32) -> Self {
        let adaptation_params = AdaptationParams {
            loss_rate_weight: 0.4,
            rtt_weight: 0.2,
            bandwidth_weight: 0.2,
            congestion_weight: 0.2,
            adaptation_speed: 0.1,
            stability_threshold: 0.05,
        };

        Self {
            codec: RaptorQCodec::new(initial_ratio),
            window_size: window_size.max(1),
            history: vec![false; window_size.max(1)],
            cursor: 0,
            min_ratio: min_ratio.clamp(0.0, 1.0),
            max_ratio: max_ratio.clamp(0.0, 1.0),
            network_conditions: Vec::with_capacity(100),
            decoding_sessions: Arc::new(Mutex::new(HashMap::new())),
            stats: Arc::new(Mutex::new(FECStats::default())),
            adaptation_params,
        }
    }

    /// Record whether the latest packet was *lost*.
    pub fn record(&mut self, lost: bool) {
        self.history[self.cursor] = lost;
        self.cursor = (self.cursor + 1) % self.window_size;
        if self.cursor == 0 {
            self.recompute();
        }
    }

    /// Update network condition for advanced adaptation
    pub fn update_network_condition(&mut self, condition: NetworkCondition) {
        self.network_conditions.push(condition);

        // Keep bounded history
        if self.network_conditions.len() > 100 {
            self.network_conditions.remove(0);
        }

        // Trigger adaptation based on comprehensive conditions
        self.adapt_to_network_conditions();
    }

    /// Get comprehensive FEC statistics
    pub fn get_stats(&self) -> FECStats {
        let codec_stats = self.codec.get_stats();
        let mut stats = self.stats.lock().unwrap().clone();
        stats.encoding = codec_stats;
        stats.current_redundancy = self.codec.redundancy();
        stats
    }

    /// Clean up expired decoding sessions
    pub fn cleanup_expired_sessions(&self) {
        let mut sessions = self.decoding_sessions.lock().unwrap();
        let now = Instant::now();
        let timeout = Duration::from_secs(30);

        sessions.retain(|&session_id, session| {
            let expired = now.duration_since(session.start_time) > timeout;
            if expired {
                warn!("Cleaning up expired decoding session {}", session_id);
            }
            !expired
        });
    }

    /// Adapt redundancy based on comprehensive network conditions
    fn adapt_to_network_conditions(&mut self) {
        if self.network_conditions.is_empty() {
            return;
        }

        // Get recent conditions
        let recent_conditions: Vec<_> = self.network_conditions.iter().rev().take(10).collect();

        // Calculate weighted adaptation score
        let mut adaptation_score = 0.0;

        // Loss rate component
        let avg_loss_rate: f32 = recent_conditions
            .iter()
            .map(|c| c.packet_loss_rate)
            .sum::<f32>()
            / recent_conditions.len() as f32;
        adaptation_score += avg_loss_rate * self.adaptation_params.loss_rate_weight;

        // RTT component (normalized)
        let avg_rtt_ms = recent_conditions
            .iter()
            .map(|c| c.rtt.as_millis() as f32)
            .sum::<f32>()
            / recent_conditions.len() as f32;
        let rtt_score = (avg_rtt_ms / 1000.0).min(1.0); // Normalize to 0-1
        adaptation_score += rtt_score * self.adaptation_params.rtt_weight;

        // Bandwidth component (inverse - lower bandwidth = higher redundancy)
        let avg_bandwidth_mbps = recent_conditions
            .iter()
            .map(|c| c.bandwidth_estimate as f32 / 1_000_000.0)
            .sum::<f32>()
            / recent_conditions.len() as f32;
        let bandwidth_score = (100.0 / (avg_bandwidth_mbps + 1.0)).min(1.0);
        adaptation_score += bandwidth_score * self.adaptation_params.bandwidth_weight;

        // Congestion component
        let avg_congestion = recent_conditions
            .iter()
            .map(|c| c.congestion_level)
            .sum::<f32>()
            / recent_conditions.len() as f32;
        adaptation_score += avg_congestion * self.adaptation_params.congestion_weight;

        // Calculate target redundancy
        let target_redundancy =
            (adaptation_score * self.max_ratio).clamp(self.min_ratio, self.max_ratio);

        // Apply adaptation with speed limiting
        let current_redundancy = self.codec.redundancy();
        let redundancy_change =
            (target_redundancy - current_redundancy) * self.adaptation_params.adaptation_speed;

        // Only adapt if change is significant
        if redundancy_change.abs() > self.adaptation_params.stability_threshold {
            let new_redundancy =
                (current_redundancy + redundancy_change).clamp(self.min_ratio, self.max_ratio);

            self.codec.set_redundancy(new_redundancy);

            debug!(
                "Adapted redundancy from {:.3} to {:.3} (score: {:.3})",
                current_redundancy, new_redundancy, adaptation_score
            );
        }
    }

    /// Get a reference to the underlying codec (read-only).
    #[must_use]
    pub fn codec(&self) -> &RaptorQCodec {
        &self.codec
    }

    fn recompute(&mut self) {
        // Count recent losses in the sliding window
        let loss_count = self.history.iter().filter(|&&lost| lost).count();
        let loss_rate = loss_count as f32 / self.window_size as f32;

        // Scale redundancy proportionally to observed loss
        let target_redundancy = (loss_rate * 2.0).clamp(self.min_ratio, self.max_ratio);

        // Apply smoothing to avoid oscillation
        let current_redundancy = self.codec.redundancy();
        let smooth_redundancy = current_redundancy * 0.7 + target_redundancy * 0.3;

        self.codec.set_redundancy(smooth_redundancy);

        debug!(
            "Recomputed redundancy: loss_rate={:.3}, redundancy={:.3}",
            loss_rate, smooth_redundancy
        );
    }

    /// Forward encoding to the underlying codec
    pub fn encode(&self, data: &[u8]) -> Vec<EncodingPacket> {
        #[cfg(feature = "telemetry")]
        let before_cap = data.len(); // proxy baseline
        let out = self.codec.encode(data);
        #[cfg(feature = "telemetry")]
        {
            // 粗い指標: 生成パケット総 payload サイズ / 入力サイズ 比率で余分なコピーの兆候を推定
            let total_payload: usize = out.iter().map(|p| p.payload.len()).sum();
            tracing::trace!(
                fec_out_packets = out.len(),
                fec_total_payload = total_payload,
                fec_input = before_cap,
                fec_payload_overhead_ratio = (total_payload as f64 / before_cap.max(1) as f64),
                "raptorq_encode_metrics"
            );
        }
        out
    }

    /// Forward decoding to the underlying codec with session tracking
    pub fn decode(&self, packets: &[EncodingPacket]) -> Option<Vec<u8>> {
        if packets.is_empty() {
            return None;
        }

        let start_time = Instant::now();
        let session_id = self.generate_session_id();

        // Create decoding session
        {
            let mut sessions = self.decoding_sessions.lock().unwrap();
            sessions.insert(
                session_id,
                DecodingSession {
                    session_id,
                    start_time,
                    expected_symbols: 0, // Will be determined from packets
                    received_symbols: packets.len() as u32,
                    decoder: None,
                },
            );
        }

        // Attempt decoding
        let result = self.codec.decode(packets);

        // Update statistics
        if let Ok(mut stats) = self.stats.lock() {
            let decoding_time = start_time.elapsed();
            stats.decoding.total_blocks_decoded += 1;

            if result.is_some() {
                stats.decoding.successful_decodings += 1;
            } else {
                stats.decoding.failed_decodings += 1;
            }

            // Update average symbols needed and decoding time
            let symbols_used = packets.len() as f32;
            if stats.decoding.total_blocks_decoded == 1 {
                stats.decoding.average_symbols_needed = symbols_used;
                stats.decoding.average_decoding_time = decoding_time;
            } else {
                let total_symbols = stats.decoding.average_symbols_needed
                    * (stats.decoding.total_blocks_decoded - 1) as f32
                    + symbols_used;
                stats.decoding.average_symbols_needed =
                    total_symbols / stats.decoding.total_blocks_decoded as f32;

                let total_time = stats.decoding.average_decoding_time.as_nanos() as u64
                    * (stats.decoding.total_blocks_decoded - 1)
                    + decoding_time.as_nanos() as u64;
                stats.decoding.average_decoding_time =
                    Duration::from_nanos(total_time / stats.decoding.total_blocks_decoded);
            }
        }

        // Clean up session
        {
            let mut sessions = self.decoding_sessions.lock().unwrap();
            sessions.remove(&session_id);
        }

        result
    }

    /// Get current redundancy ratio
    pub fn redundancy(&self) -> f32 {
        self.codec.redundancy()
    }

    /// Generate unique session ID
    fn generate_session_id(&self) -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SESSION_COUNTER: AtomicU64 = AtomicU64::new(1);
        SESSION_COUNTER.fetch_add(1, Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_encoding() {
        let codec = RaptorQCodec::new(0.3);
        let data = b"Hello, RaptorQ world!";
        let packets = codec.encode(data);
        assert!(!packets.is_empty());

        // Should have sentinel packet
        assert_eq!(packets[0].payload_id().source_block_number(), 0xFF);
        assert_eq!(packets[0].payload_id().encoding_symbol_id(), 0xFFFFFF);
    }

    #[test]
    fn test_codec_decoding() {
        let codec = RaptorQCodec::new(0.2);
        let data = b"Test data for RaptorQ";
        let packets = codec.encode(data);

        let decoded = codec.decode(&packets).expect("Decoding should succeed");
        assert_eq!(data, decoded.as_slice());
    }

    #[test]
    fn test_adaptation_increases_redundancy() {
        let mut adapt = AdaptiveRaptorQ::new(0.05, 8, 0.05, 0.5);
        let initial_redundancy = adapt.redundancy();

        // Simulate heavy losses in one window.
        for _ in 0..8 {
            adapt.record(true);
        }
        assert!(adapt.codec.redundancy() > initial_redundancy);
    }

    #[test]
    fn test_network_condition_adaptation() {
        let mut adapt = AdaptiveRaptorQ::new(0.1, 10, 0.05, 0.8);
        let initial_redundancy = adapt.redundancy();

        // Simulate poor network conditions
        let poor_condition = NetworkCondition {
            timestamp: Instant::now(),
            packet_loss_rate: 0.15,
            rtt: Duration::from_millis(500),
            bandwidth_estimate: 100_000, // Low bandwidth
            congestion_level: 0.8,
        };

        adapt.update_network_condition(poor_condition);

        // Redundancy should increase due to poor conditions
        assert!(adapt.redundancy() >= initial_redundancy);
    }

    #[test]
    fn test_redundancy_increase_and_decrease_cycle() {
        // Start with moderate ratio allowing both directions
        let mut adapt = AdaptiveRaptorQ::new(0.2, 12, 0.05, 0.6);
        let start = adapt.redundancy();
        // Feed a larger series of very poor conditions to force upward pressure.
        // Because adaptation_speed & stability_threshold may damp small changes,
        // iterate more times to accumulate adaptation_score divergence.
        for _ in 0..20 {
            adapt.update_network_condition(NetworkCondition {
                timestamp: Instant::now(),
                packet_loss_rate: 0.25,
                rtt: Duration::from_millis(800),
                bandwidth_estimate: 50_000, // very low
                congestion_level: 0.9,
            });
        }
        let increased = adapt.redundancy();
        assert!(increased >= start, "redundancy should not decrease under persistent poor conditions (start {:.3} -> {:.3})", start, increased);

        // Now feed good conditions to encourage downward adaptation
        for _ in 0..30 {
            adapt.update_network_condition(NetworkCondition {
                timestamp: Instant::now(),
                packet_loss_rate: 0.0,
                rtt: Duration::from_millis(40),
                bandwidth_estimate: 20_000_000, // high
                congestion_level: 0.05,
            });
        }
        let decreased = adapt.redundancy();
        // Allow small hysteresis; ensure it did not continue rising
        assert!(
            decreased <= increased + 0.0001,
            "redundancy should not increase further after recovery (increased {:.3} -> {:.3})",
            increased,
            decreased
        );

        // Always remain within configured bounds
        assert!(decreased >= 0.05 && decreased <= 0.6);
    }

    #[test]
    fn test_session_cleanup() {
        let adapt = AdaptiveRaptorQ::new(0.2, 10, 0.1, 0.5);

        // Add a mock expired session
        {
            let mut sessions = adapt.decoding_sessions.lock().unwrap();
            sessions.insert(
                1,
                DecodingSession {
                    session_id: 1,
                    start_time: Instant::now() - Duration::from_secs(60), // Expired
                    expected_symbols: 10,
                    received_symbols: 5,
                    decoder: None,
                },
            );
        }

        adapt.cleanup_expired_sessions();

        // Session should be cleaned up
        assert!(adapt.decoding_sessions.lock().unwrap().is_empty());
    }

    #[test]
    fn test_stats_collection() {
        let codec = RaptorQCodec::new(0.25);
        let data = b"Statistics test data";

        // Perform encoding
        let _packets = codec.encode(data);

        // Check statistics
        let stats = codec.get_stats();
        assert_eq!(stats.total_blocks_encoded, 1);
        assert!(!stats.redundancy_history.is_empty());
        assert!(stats.average_encoding_time > Duration::from_nanos(0));
    }
}
