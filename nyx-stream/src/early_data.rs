//! 0-RTT Early Data and Anti-Replay Protection for Nyx Protocol v1.0
//!
//! This module implements the complete Early-Data and 0-RTT Reception requirements
//! as specified in `spec/Nyx_Protocol_v1.0_Spec_EN.md` Section 2.1.
//!
//! ## Key Security Features
//!
//! - **Direction Identifier**: 32-bit IDs prevent nonce overlap between half-duplex directions
//! - **Anti-Replay Window**: Sliding window of size 2^20 for per-direction nonce tracking  
//! - **Early Data Scope**: 0-RTT application data accepted after client first CRYPTO message
//! - **Rekey Interaction**: Nonces reset to zero, anti-replay window reset on rekey
//! - **Telemetry Integration**: Comprehensive counters for replay drops and early-data acceptance
//! - **Security Validation**: Strict bounds checking and DoS protection

#![forbid(unsafe_code)]

use crate::errors::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

/// Anti-replay window size as specified in the protocol (2^20 = 1,048,576)
pub const ANTI_REPLAY_WINDOW_SIZE: u64 = 1 << 20;

/// Maximum early data payload size (64KB) to prevent DoS attacks
pub const MAX_EARLY_DATA_SIZE: usize = 64 * 1024;

/// Maximum total early data per connection (1MB)
pub const MAX_TOTAL_EARLY_DATA: usize = 1024 * 1024;

/// Direction identifiers for preventing nonce overlap in AEAD construction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DirectionId(pub u32);

impl DirectionId {
    /// Client to Server direction (Initiator to Responder)
    pub const CLIENT_TO_SERVER: DirectionId = DirectionId(0x00000001);
    /// Server to Client direction (Responder to Initiator)  
    pub const SERVER_TO_CLIENT: DirectionId = DirectionId(0x00000002);

    /// Create a new direction identifier
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the raw direction ID value
    pub fn value(&self) -> u32 {
        self.0
    }

    /// Get the opposite direction
    pub fn opposite(&self) -> Self {
        match *self {
            Self::CLIENT_TO_SERVER => Self::SERVER_TO_CLIENT,
            Self::SERVER_TO_CLIENT => Self::CLIENT_TO_SERVER,
            DirectionId(id) => DirectionId(!id), // Bitwise NOT for custom IDs
        }
    }
}

impl std::fmt::Display for DirectionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::CLIENT_TO_SERVER => write!(f, "C→S"),
            Self::SERVER_TO_CLIENT => write!(f, "S→C"),
            DirectionId(id) => write!(f, "Dir({id:08x})"),
        }
    }
}

/// Nonce value for AEAD operations with anti-replay protection
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Nonce(pub u64);

impl Nonce {
    /// Create a new nonce
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Get the raw nonce value
    pub fn value(&self) -> u64 {
        self.0
    }

    /// Increment the nonce (for sending)
    pub fn increment(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }

    /// Create nonce for AEAD with direction identifier
    pub fn to_aead_nonce(&self, direction: DirectionId) -> [u8; 12] {
        let mut nonce = [0u8; 12];
        nonce[0..4].copy_from_slice(&direction.value().to_be_bytes());
        nonce[4..12].copy_from_slice(&self.0.to_be_bytes());
        nonce
    }
}

// Default is derived as Nonce(0)

/// Anti-replay protection using an efficient sliding window implementation
#[derive(Debug)]
pub struct AntiReplayWindow {
    /// Direction identifier for this window
    direction_id: DirectionId,
    /// Current window base (highest seen nonce)
    window_base: u64,
    /// Set of seen nonces within the window
    seen_nonces: BTreeSet<u64>,
    /// Maximum window size (2^20 for specification compliance)
    window_size: u64,
    /// Total nonces processed
    total_processed: u64,
    /// Number of replay attempts blocked
    replay_blocks: u64,
    /// Creation timestamp for metrics
    created_at: Instant,
    /// Last reset timestamp
    last_reset: Option<Instant>,
}

impl AntiReplayWindow {
    /// Create new anti-replay window for a direction
    pub fn new(direction_id: DirectionId) -> Self {
        Self {
            direction_id,
            window_base: 0,
            seen_nonces: BTreeSet::new(),
            window_size: ANTI_REPLAY_WINDOW_SIZE,
            total_processed: 0,
            replay_blocks: 0,
            created_at: Instant::now(),
            last_reset: None,
        }
    }

    /// Create anti-replay window with custom size (for testing)
    pub fn with_size(direction_id: DirectionId, window_size: u64) -> Self {
        Self {
            direction_id,
            window_base: 0,
            seen_nonces: BTreeSet::new(),
            window_size,
            total_processed: 0,
            replay_blocks: 0,
            created_at: Instant::now(),
            last_reset: None,
        }
    }

    /// Check if a nonce is valid and not replayed
    /// Returns true if the nonce should be accepted
    pub fn check_and_update(&mut self, nonce: Nonce) -> bool {
        let nonce_value = nonce.value();
        self.total_processed += 1;

        // SECURITY: Special case - reject nonce 0 if we've processed higher nonces
        if nonce_value == 0 && self.window_base > 0 {
            debug!(
                direction = %self.direction_id,
                nonce = nonce_value,
                window_base = self.window_base,
                "SECURITY: Rejecting nonce 0 after higher nonces processed"
            );
            self.replay_blocks += 1;
            return false;
        }

        // SECURITY: Reject nonces that are too far in the future
        if nonce_value > self.window_base + self.window_size {
            debug!(
                direction = %self.direction_id,
                nonce = nonce_value,
                window_base = self.window_base,
                window_size = self.window_size,
                "SECURITY: Rejecting nonce too far in future"
            );
            self.replay_blocks += 1;
            return false;
        }

        // SECURITY: Reject nonces that are too old (outside window)
        if nonce_value + self.window_size < self.window_base {
            debug!(
                direction = %self.direction_id,
                nonce = nonce_value,
                window_base = self.window_base,
                "SECURITY: Rejecting nonce outside replay window (too old)"
            );
            self.replay_blocks += 1;
            return false;
        }

        // SECURITY: Check for replay
        if self.seen_nonces.contains(&nonce_value) {
            warn!(
                direction = %self.direction_id,
                nonce = nonce_value,
                "SECURITY: Replay attack detected - nonce already seen"
            );
            self.replay_blocks += 1;
            return false;
        }

        // Accept the nonce and update window state
        self.accept_nonce(nonce_value);
        true
    }

    /// Accept a nonce and update the window state
    fn accept_nonce(&mut self, nonce_value: u64) {
        // Update window base if this nonce is newer
        if nonce_value > self.window_base {
            self.window_base = nonce_value;

            // Clean up old nonces outside the new window
            let cutoff = self.window_base.saturating_sub(self.window_size);
            self.seen_nonces = self.seen_nonces.split_off(&cutoff);
        }

        // Add the nonce to seen set
        self.seen_nonces.insert(nonce_value);

        debug!(
            direction = %self.direction_id,
            nonce = nonce_value,
            window_base = self.window_base,
            seen_count = self.seen_nonces.len(),
            "Accepted nonce and updated replay window"
        );
    }

    /// Reset the window (used during rekey operations)
    pub fn reset(&mut self) {
        info!(
            direction = %self.direction_id,
            "SECURITY: Resetting anti-replay window for rekey"
        );

        self.window_base = 0;
        self.seen_nonces.clear();
        self.last_reset = Some(Instant::now());
    }

    /// Get window statistics for telemetry
    pub fn stats(&self) -> AntiReplayStats {
        AntiReplayStats {
            direction_id: self.direction_id,
            window_base: self.window_base,
            seen_count: self.seen_nonces.len(),
            window_size: self.window_size,
            total_processed: self.total_processed,
            replay_blocks: self.replay_blocks,
            uptime: self.created_at.elapsed(),
            last_reset: self.last_reset.map(|t| t.elapsed()),
        }
    }
}

/// Statistics for anti-replay window telemetry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiReplayStats {
    pub direction_id: DirectionId,
    pub window_base: u64,
    pub seen_count: usize,
    pub window_size: u64,
    pub total_processed: u64,
    pub replay_blocks: u64,
    pub uptime: Duration,
    pub last_reset: Option<Duration>,
}

/// Early data state machine for 0-RTT handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EarlyDataState {
    /// No early data allowed (initial state)
    Disabled,
    /// Early data allowed after first CRYPTO message
    Enabled,
    /// Early data window closed (handshake complete)
    Completed,
    /// Early data permanently disabled due to security concerns
    SecurityDisabled,
}

/// Comprehensive 0-RTT and Early Data manager
#[derive(Debug)]
pub struct EarlyDataManager {
    /// Direction-specific anti-replay windows
    windows: HashMap<DirectionId, Arc<Mutex<AntiReplayWindow>>>,

    /// Current early data state
    state: EarlyDataState,

    /// Telemetry metrics
    metrics: Arc<Mutex<EarlyDataMetrics>>,

    /// Configuration limits
    max_early_data_size: usize,
    max_total_early_data: usize,

    /// Total early data received in this session
    total_early_data_received: usize,

    /// Start time for session metrics
    session_start: Instant,
}

impl EarlyDataManager {
    /// Create a new early data manager
    pub fn new() -> Self {
        let mut windows = HashMap::new();

        // Create windows for both directions
        windows.insert(
            DirectionId::CLIENT_TO_SERVER,
            Arc::new(Mutex::new(AntiReplayWindow::new(
                DirectionId::CLIENT_TO_SERVER,
            ))),
        );
        windows.insert(
            DirectionId::SERVER_TO_CLIENT,
            Arc::new(Mutex::new(AntiReplayWindow::new(
                DirectionId::SERVER_TO_CLIENT,
            ))),
        );

        Self {
            windows,
            state: EarlyDataState::Disabled,
            metrics: Arc::new(Mutex::new(EarlyDataMetrics::default())),
            max_early_data_size: MAX_EARLY_DATA_SIZE,
            max_total_early_data: MAX_TOTAL_EARLY_DATA,
            total_early_data_received: 0,
            session_start: Instant::now(),
        }
    }

    /// Enable early data after first CRYPTO message received
    pub fn enable_early_data(&mut self) -> Result<()> {
        if self.state == EarlyDataState::SecurityDisabled {
            return Err(Error::Protocol(
                "Early data permanently disabled due to security concerns".to_string(),
            ));
        }

        info!("Enabling early data acceptance after first CRYPTO message");
        self.state = EarlyDataState::Enabled;

        // Update metrics
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.early_data_enabled_count += 1;
            metrics.last_state_change = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default(),
            );
        }

        Ok(())
    }

    /// Complete handshake and transition to normal operation
    pub fn complete_handshake(&mut self) {
        info!("Handshake complete, transitioning early data to completed state");
        self.state = EarlyDataState::Completed;

        // Update metrics
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.handshake_completed_count += 1;
            metrics.session_duration = Some(self.session_start.elapsed());
            metrics.last_state_change = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default(),
            );
        }
    }

    /// Disable early data permanently due to security concerns
    pub fn disable_for_security(&mut self, reason: String) {
        error!(reason = %reason, "Permanently disabling early data due to security concerns");
        self.state = EarlyDataState::SecurityDisabled;

        // Update metrics
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.security_disable_count += 1;
            metrics.security_disable_reasons.insert(reason);
            metrics.last_state_change = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default(),
            );
        }
    }

    /// Validate and process early data packet
    pub fn validate_early_data(
        &mut self,
        direction: DirectionId,
        nonce: Nonce,
        data: &[u8],
    ) -> Result<bool> {
        // SECURITY: Check if early data is allowed in current state
        if self.state != EarlyDataState::Enabled {
            debug!(
                state = ?self.state,
                direction = %direction,
                "Early data not allowed in current state"
            );

            if let Ok(mut metrics) = self.metrics.lock() {
                metrics.early_data_rejected_count += 1;
                metrics
                    .rejection_reasons
                    .insert("state_not_enabled".to_string());
            }

            return Ok(false);
        }

        // SECURITY: Validate individual packet size
        if data.len() > self.max_early_data_size {
            error!(
                size = data.len(),
                max_size = self.max_early_data_size,
                direction = %direction,
                "SECURITY: Early data packet exceeds maximum allowed size"
            );

            if let Ok(mut metrics) = self.metrics.lock() {
                metrics.early_data_rejected_count += 1;
                metrics
                    .rejection_reasons
                    .insert("oversized_packet".to_string());
            }

            return Err(Error::Protocol(format!(
                "SECURITY: Early data packet size {} exceeds maximum {}",
                data.len(),
                self.max_early_data_size
            )));
        }

        // SECURITY: Validate total session early data
        if self.total_early_data_received + data.len() > self.max_total_early_data {
            error!(
                current_total = self.total_early_data_received,
                new_data = data.len(),
                max_total = self.max_total_early_data,
                direction = %direction,
                "SECURITY: Total early data would exceed session limit"
            );

            if let Ok(mut metrics) = self.metrics.lock() {
                metrics.early_data_rejected_count += 1;
                metrics
                    .rejection_reasons
                    .insert("total_session_limit_exceeded".to_string());
            }

            return Ok(false);
        }

        // SECURITY: Get and check anti-replay window
        let window = self.windows.get(&direction).ok_or_else(|| {
            Error::Protocol(format!("No anti-replay window for direction {direction}"))
        })?;

        let nonce_valid = {
            let mut window_guard = window.lock().map_err(|_| {
                Error::Protocol("Failed to acquire anti-replay window lock".to_string())
            })?;
            window_guard.check_and_update(nonce)
        };

        if !nonce_valid {
            warn!(
                direction = %direction,
                nonce = nonce.value(),
                "SECURITY: Early data rejected due to anti-replay protection"
            );

            if let Ok(mut metrics) = self.metrics.lock() {
                metrics.replay_drops += 1;
                metrics.early_data_rejected_count += 1;
                metrics
                    .rejection_reasons
                    .insert("replay_protection".to_string());
            }

            return Ok(false);
        }

        // Accept the early data
        self.total_early_data_received += data.len();

        // Update success metrics
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.early_data_accepted_count += 1;
            metrics.total_early_data_bytes += data.len();
            metrics.last_early_data_timestamp = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default(),
            );
        }

        info!(
            direction = %direction,
            nonce = nonce.value(),
            size = data.len(),
            total_session = self.total_early_data_received,
            "Early data packet accepted and processed"
        );

        Ok(true)
    }

    /// Reset all anti-replay windows for rekey operation
    pub fn reset_for_rekey(&mut self) -> Result<()> {
        info!("SECURITY: Resetting all anti-replay windows for rekey operation");

        // Reset all direction windows
        for (direction, window) in &self.windows {
            let mut window_guard = window.lock().map_err(|_| {
                Error::Protocol(format!("Failed to lock window for direction {direction}"))
            })?;
            window_guard.reset();
        }

        // Reset session early data counter
        self.total_early_data_received = 0;

        // Update metrics
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.rekey_count += 1;
            metrics.last_rekey_timestamp = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default(),
            );
        }

        Ok(())
    }

    /// Get current early data state
    pub fn state(&self) -> EarlyDataState {
        self.state
    }

    /// Get comprehensive metrics for telemetry and monitoring
    pub fn metrics(&self) -> Result<EarlyDataMetrics> {
        let metrics = self
            .metrics
            .lock()
            .map_err(|_| Error::Protocol("Failed to acquire metrics lock".to_string()))?
            .clone();

        Ok(metrics)
    }

    /// Get anti-replay window statistics for all directions
    pub fn window_stats(&self) -> Result<HashMap<DirectionId, AntiReplayStats>> {
        let mut stats = HashMap::new();

        for (direction, window) in &self.windows {
            let window_guard = window.lock().map_err(|_| {
                Error::Protocol(format!("Failed to lock window for direction {direction}"))
            })?;
            stats.insert(*direction, window_guard.stats());
        }

        Ok(stats)
    }

    /// Get session statistics
    pub fn session_stats(&self) -> SessionStats {
        SessionStats {
            state: self.state,
            total_early_data_received: self.total_early_data_received,
            max_early_data_size: self.max_early_data_size,
            max_total_early_data: self.max_total_early_data,
            session_uptime: self.session_start.elapsed(),
        }
    }
}

impl Default for EarlyDataManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Session statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub state: EarlyDataState,
    pub total_early_data_received: usize,
    pub max_early_data_size: usize,
    pub max_total_early_data: usize,
    pub session_uptime: Duration,
}

/// Comprehensive telemetry metrics for early data operations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EarlyDataMetrics {
    /// Number of early data packets accepted
    pub early_data_accepted_count: u64,

    /// Number of early data packets rejected
    pub early_data_rejected_count: u64,

    /// Total bytes of early data accepted
    pub total_early_data_bytes: usize,

    /// Number of replay attacks detected and blocked
    pub replay_drops: u64,

    /// Number of times early data was enabled
    pub early_data_enabled_count: u64,

    /// Number of handshake completions
    pub handshake_completed_count: u64,

    /// Number of security-based disables
    pub security_disable_count: u64,

    /// Number of rekey operations performed
    pub rekey_count: u64,

    /// Detailed rejection reasons for analysis
    pub rejection_reasons: HashSet<String>,

    /// Security disable reasons for auditing
    pub security_disable_reasons: HashSet<String>,

    /// Timestamp of last early data acceptance
    pub last_early_data_timestamp: Option<Duration>,

    /// Timestamp of last rekey operation
    pub last_rekey_timestamp: Option<Duration>,

    /// Timestamp of last state change
    pub last_state_change: Option<Duration>,

    /// Session duration (set when handshake completes)
    pub session_duration: Option<Duration>,
}

// Default is derived

/// Helper utilities for AEAD nonce construction with direction identifiers
pub struct NonceConstructor;

impl NonceConstructor {
    /// Construct AEAD nonce from direction and sequence number
    pub fn construct_aead_nonce(direction: DirectionId, nonce: Nonce) -> [u8; 12] {
        nonce.to_aead_nonce(direction)
    }

    /// Validate nonce format and constraints
    pub fn validate_nonce(nonce: Nonce) -> Result<()> {
        // SECURITY: Ensure nonce is not at maximum value (prevent overflow)
        if nonce.value() == u64::MAX {
            return Err(Error::Protocol(
                "SECURITY: Nonce at maximum value, overflow risk detected".to_string(),
            ));
        }

        Ok(())
    }

    /// Create initial nonce for new connection
    pub fn initial_nonce() -> Nonce {
        Nonce::new(0)
    }

    /// Create next nonce in sequence
    pub fn next_nonce(current: Nonce) -> Result<Nonce> {
        Self::validate_nonce(current)?;
        Ok(Nonce::new(current.value().wrapping_add(1)))
    }
}
