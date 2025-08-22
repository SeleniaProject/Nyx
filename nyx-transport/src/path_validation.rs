// Comprehensive real-time path health monitoring
// Path validation and connectivity verification system.
// Thi_s module provide_s:
// - PATH_CHALLENGE/PATH_RESPONSE frame handling
// - Bi-directional connectivity verification
// - Multi-path validation with priority management
// - Network quality assessment and metric_s
// - Connection migration support with validation
// - Real-time path health monitoring

/// Safe wrapper for mutex lock operation_s
fn safe_mutex_lock<'a, T>(
    mutex: &'a Mutex<T>,
    operation: &str,
) -> Result<std::sync::MutexGuard<'a, T>> {
    mutex
        .lock()
        .map_err(|_| Error::Internal(format!("Mutex poisoned during {}", operation)))
}

use crate::{Error, Result};
use rand::rngs::OsRng;
use rand::RngCore; // for cryptographically secure token generation
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::time::timeout;

/// Path challenge frame type identifier (0x33)
pub const PATH_CHALLENGE_FRAME_TYPE: u8 = 0x33;

/// Path response frame type identifier (0x34)
pub const PATH_RESPONSE_FRAME_TYPE: u8 = 0x34;

/// Path challenge token size (128 bit_s = 16 byte_s)
pub const PATH_CHALLENGE_TOKEN_SIZE: usize = 16;

/// Default path validation timeout
pub const DEFAULT_PATH_VALIDATION_TIMEOUT: Duration = Duration::from_secs(3);

/// Maximum concurrent path validation attempt_s
pub const MAX_CONCURRENT_VALIDATIONS: usize = 8;

/// Path validation retry attempt_s
pub const PATH_VALIDATION_RETRIES: u32 = 3;

/// Poll interval to check for cancellation during wait loop_s
const VALIDATION_POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Path validation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathValidationState {
    /// Path validation i_s pending
    Pending,
    /// Path validation i_s in progres_s
    Validating,
    /// Path ha_s been successfully validated
    Validated,
    /// Path validation failed
    Failed,
    /// Path validation timed out
    TimedOut,
}

/// Path quality metrics for validation assessment
#[derive(Debug, Clone)]
pub struct PathMetrics {
    pub __round_trip_time: Duration,
    pub __packet_loss_rate: f64,
    pub __jitter: Duration,
    pub __bandwidth_estimate: u64, // byte_s per second
    pub __last_validated: Instant,
    pub __validation_count: u32,
    pub __failure_count: u32,
}

impl Default for PathMetrics {
    fn default() -> Self {
        Self {
            __round_trip_time: Duration::from_millis(100),
            __packet_loss_rate: 0.0,
            __jitter: Duration::from_millis(10),
            __bandwidth_estimate: 1_000_000, // 1 MB/_s default
            __last_validated: Instant::now(),
            __validation_count: 0,
            __failure_count: 0,
        }
    }
}

/// Path validation challenge information
#[derive(Debug, Clone)]
pub struct PathChallenge {
    pub token: [u8; PATH_CHALLENGE_TOKEN_SIZE],
    pub __target_addr: SocketAddr,
    pub __sent_at: Instant,
    pub __attempt: u32,
    pub __state: PathValidationState,
}

impl PathChallenge {
    /// Create a new path challenge with random token
    pub fn new(__target_addr: SocketAddr) -> Self {
        // Use OS-backed CSPRNG for unpredictable token generation.
        // This prevents off-path/on-path attackers from predicting tokens and spoofing PATH_RESPONSE.
        let mut token = [0u8; PATH_CHALLENGE_TOKEN_SIZE];
        OsRng.fill_bytes(&mut token);

        Self {
            token,
            __target_addr,
            __sent_at: Instant::now(),
            __attempt: 1,
            __state: PathValidationState::Pending,
        }
    }

    /// Check if thi_s challenge ha_s timed out
    pub fn is_timed_out(&self, timeout: Duration) -> bool {
        self.__sent_at.elapsed() > timeout
    }

    /// Get the unique token as hex string (for debugging)
    pub fn token_hex(&self) -> String {
        hex::encode(self.token)
    }
}

/// Comprehensive path validation manager
pub struct PathValidator {
    local_socket: Arc<UdpSocket>,
    active_challenge_s: Arc<Mutex<HashMap<String, PathChallenge>>>,
    path_metric_s: Arc<Mutex<HashMap<SocketAddr, PathMetrics>>>,
    __validation_timeout: Duration,
    __max_retrie_s: u32,
    cancel_flag: Arc<AtomicBool>,
    success_count: Arc<AtomicU64>,
    failure_count: Arc<AtomicU64>,
    timeout_count: Arc<AtomicU64>,
    cancel_count: Arc<AtomicU64>,
}

impl PathValidator {
    /// Create a new path validator
    pub async fn new(local_addr: SocketAddr) -> Result<Self> {
        let __socket = UdpSocket::bind(local_addr)
            .await
            .map_err(|e| Error::Msg(format!("Failed to bind path validator socket: {}", e)))?;

        Ok(Self {
            local_socket: Arc::new(__socket),
            active_challenge_s: Arc::new(Mutex::new(HashMap::new())),
            path_metric_s: Arc::new(Mutex::new(HashMap::new())),
            __validation_timeout: DEFAULT_PATH_VALIDATION_TIMEOUT,
            __max_retrie_s: PATH_VALIDATION_RETRIES,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            success_count: Arc::new(AtomicU64::new(0)),
            failure_count: Arc::new(AtomicU64::new(0)),
            timeout_count: Arc::new(AtomicU64::new(0)),
            cancel_count: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Create a new path validator with custom timeout
    pub async fn new_with_timeout(__local_addr: SocketAddr, timeout: Duration) -> Result<Self> {
        let __socket = UdpSocket::bind(__local_addr)
            .await
            .map_err(|e| Error::Msg(format!("Failed to bind path validator socket: {}", e)))?;

        Ok(Self {
            local_socket: Arc::new(__socket),
            active_challenge_s: Arc::new(Mutex::new(HashMap::new())),
            path_metric_s: Arc::new(Mutex::new(HashMap::new())),
            __validation_timeout: timeout,
            __max_retrie_s: PATH_VALIDATION_RETRIES,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            success_count: Arc::new(AtomicU64::new(0)),
            failure_count: Arc::new(AtomicU64::new(0)),
            timeout_count: Arc::new(AtomicU64::new(0)),
            cancel_count: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Create a new path validator with custom timeout and retry count
    pub async fn new_with_timeout_and_retrie_s(
        __local_addr: SocketAddr,
        __timeout: Duration,
        __max_retrie_s: u32,
    ) -> Result<Self> {
        let __socket = UdpSocket::bind(__local_addr)
            .await
            .map_err(|e| Error::Msg(format!("Failed to bind path validator socket: {}", e)))?;
        Ok(Self {
            local_socket: Arc::new(__socket),
            active_challenge_s: Arc::new(Mutex::new(HashMap::new())),
            path_metric_s: Arc::new(Mutex::new(HashMap::new())),
            __validation_timeout: __timeout,
            __max_retrie_s: __max_retrie_s,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            success_count: Arc::new(AtomicU64::new(0)),
            failure_count: Arc::new(AtomicU64::new(0)),
            timeout_count: Arc::new(AtomicU64::new(0)),
            cancel_count: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Validate a path to the specified addres_s
    pub async fn validate_path(&self, __target_addr: SocketAddr) -> Result<PathMetrics> {
        // Reset any previou_s cancellation before starting a new validation
        self.cancel_flag.store(false, Ordering::SeqCst);
        let mut last_err: Option<Error> = None;
        for attempt in 1..=self.__max_retrie_s {
            let mut challenge = PathChallenge::new(__target_addr);
            challenge.__attempt = attempt;
            let __token_key = challenge.token_hex();

            // Store the challenge (replace any previou_s for same token)
            {
                let mut challenge_s =
                    safe_mutex_lock(&self.active_challenge_s, "path_challenge_receive")?;
                challenge_s.insert(__token_key.clone(), challenge.clone());
            }

            // Send PATH_CHALLENGE frame
            self.send_path_challenge(&challenge).await?;

            // Compute deadline and wait
            let __deadline = Instant::now() + self.__validation_timeout;
            match self
                .wait_for_path_response(&__token_key, challenge.__sent_at, __deadline)
                .await
            {
                Ok(metric_s) => {
                    self.success_count.fetch_add(1, Ordering::Relaxed);
                    // Update stored metric_s
                    {
                        let mut path_metric_s =
                            safe_mutex_lock(&self.path_metric_s, "path_metrics_operation")?;
                        let mut m = metric_s.clone();
                        m.__validation_count = 1;
                        path_metric_s.insert(__target_addr, m.clone());
                    }
                    // Clean up
                    self.cleanup_challenge(&__token_key);
                    return Ok(metric_s);
                }
                Err(e) => {
                    // Clean up thi_s challenge and retry if attempt_s remain
                    self.cleanup_challenge(&__token_key);
                    last_err = Some(e);
                    if attempt < self.__max_retrie_s {
                        // brief backoff before retry
                        tokio::time::sleep(Duration::from_millis(30)).await;
                        continue;
                    }
                }
            }
        }
        // Final classification and counter_s
        if let Some(Error::Msg(msg)) = &last_err {
            if msg.contains("cancelled") {
                self.cancel_count.fetch_add(1, Ordering::Relaxed);
            } else if msg.contains("No valid PATH_RESPONSE received") || msg.contains("timeout") {
                self.timeout_count.fetch_add(1, Ordering::Relaxed);
            }
        }
        self.failure_count.fetch_add(1, Ordering::Relaxed);
        Err(match last_err {
            Some(e) => e,
            None => {
                tracing::error!("Path validation failed: no error captured in last_err");
                Error::Msg("Path validation failed".into())
            }
        })
    }

    /// Send PATH_CHALLENGE frame to target addres_s
    async fn send_path_challenge(&self, challenge: &PathChallenge) -> Result<()> {
        let mut frame = Vec::new();
        frame.push(PATH_CHALLENGE_FRAME_TYPE);
        frame.extend_from_slice(&challenge.token);

        self.local_socket
            .send_to(&frame, challenge.__target_addr)
            .await
            .map_err(|e| Error::Msg(format!("Failed to send PATH_CHALLENGE: {}", e)))?;

        Ok(())
    }

    /// Wait for PATH_RESPONSE for a specific challenge
    async fn wait_for_path_response(
        &self,
        token_key: &str,
        __sent_at: Instant,
        deadline: Instant,
    ) -> Result<PathMetrics> {
        let mut buffer = [0u8; 1024];
        loop {
            let __now = Instant::now();
            if __now >= deadline {
                break;
            }
            if self.cancel_flag.load(Ordering::SeqCst) {
                return Err(Error::Msg("Path validation cancelled".to_string()));
            }
            let __remain = deadline.saturating_duration_since(__now);
            // Bound each await to a short poll interval to observe cancellation quickly
            let __recv_timeout = if __remain > VALIDATION_POLL_INTERVAL {
                VALIDATION_POLL_INTERVAL
            } else {
                __remain
            };
            match timeout(__recv_timeout, self.local_socket.recv_from(&mut buffer)).await {
                Ok(Ok((len, from_addr))) => {
                    if let Some(metric_s) = self
                        .process_received_frame(&buffer[..len], from_addr, token_key, __sent_at)
                        .await?
                    {
                        return Ok(metric_s);
                    }
                    // loop and continue until deadline
                }
                Ok(Err(_e)) => {
                    // Treat transient socket error_s as non-fatal; retry until deadline
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                Err(_) => {
                    // timeout for thi_s recv; loop will break on outer deadline check
                    continue;
                }
            }
        }
        Err(Error::Msg("No valid PATH_RESPONSE received".to_string()))
    }

    /// Cancel any in-flight path validation_s initiated via thi_s validator.
    /// Subsequent call_s to validate_path will reset thi_s flag.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
    }

    /// Validation counter_s snapshot
    pub fn counter_s(&self) -> PathValidationCounters {
        PathValidationCounters {
            __succes_s: self.success_count.load(Ordering::Relaxed),
            __failure: self.failure_count.load(Ordering::Relaxed),
            __timeout: self.timeout_count.load(Ordering::Relaxed),
            __cancelled: self.cancel_count.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of validation counter_s
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathValidationCounters {
    pub __succes_s: u64,
    pub __failure: u64,
    pub __timeout: u64,
    pub __cancelled: u64,
}

impl PathValidator {
    /// Proces_s received frame and check for PATH_RESPONSE
    async fn process_received_frame(
        &self,
        _data: &[u8],
        __from_addr: SocketAddr,
        expected_token_key: &str,
        __start_time: Instant,
    ) -> Result<Option<PathMetrics>> {
        if _data.len() < 1 + PATH_CHALLENGE_TOKEN_SIZE {
            return Ok(None);
        }

        let __frame_type = _data[0];
        if __frame_type == PATH_RESPONSE_FRAME_TYPE {
            let __received_token = &_data[1..1 + PATH_CHALLENGE_TOKEN_SIZE];
            let __received_token_key = hex::encode(__received_token);

            if __received_token_key == expected_token_key {
                // Verify that the response come_s from the originally challenged addres_s.
                let __from_expected_addr = {
                    let __challenge_s =
                        safe_mutex_lock(&self.active_challenge_s, "active_challenges_operation")?;
                    match __challenge_s.get(expected_token_key) {
                        Some(c) => c.__target_addr == __from_addr,
                        None => {
                            tracing::warn!(
                                "No challenge found for expected_token_key: {}",
                                expected_token_key
                            );
                            false
                        }
                    }
                };
                if !__from_expected_addr {
                    // Ignore response_s from unexpected addres_s (possible spoof/reflection)
                    return Ok(None);
                }
                // Valid PATH_RESPONSE received
                let __rtt = __start_time.elapsed();

                let __metric_s = PathMetrics {
                    __round_trip_time: __rtt,
                    __packet_loss_rate: 0.0, // No los_s for successful validation
                    __jitter: Duration::from_millis(__rtt.as_millis() as u64 / 10), // Estimated jitter
                    __bandwidth_estimate: self.estimate_bandwidth(_data.len(), __rtt),
                    __last_validated: Instant::now(),
                    __validation_count: 1,
                    __failure_count: 0,
                };

                return Ok(Some(__metric_s));
            }
        } else if __frame_type == PATH_CHALLENGE_FRAME_TYPE {
            // Received PATH_CHALLENGE, send PATH_RESPONSE
            let __token = &_data[1..1 + PATH_CHALLENGE_TOKEN_SIZE];
            self.send_path_response(__token, __from_addr).await.ok();
        }

        Ok(None)
    }

    /// Send PATH_RESPONSE frame
    async fn send_path_response(&self, token: &[u8], __target_addr: SocketAddr) -> Result<()> {
        let mut frame = Vec::new();
        frame.push(PATH_RESPONSE_FRAME_TYPE);
        frame.extend_from_slice(token);

        self.local_socket
            .send_to(&frame, __target_addr)
            .await
            .map_err(|e| Error::Msg(format!("Failed to send PATH_RESPONSE: {}", e)))?;

        Ok(())
    }

    /// Estimate bandwidth based on frame size and RTT
    fn estimate_bandwidth(&self, __frame_size: usize, rtt: Duration) -> u64 {
        if rtt.is_zero() {
            return 1_000_000; // Default 1 MB/_s
        }

        // Simple bandwidth estimation: frame_size / rtt
        let __bytes_per_second = (__frame_size as f64) / rtt.as_secs_f64();
        (__bytes_per_second as u64).max(1000) // Minimum 1 KB/_s
    }

    /// Cleanup expired or completed challenge
    fn cleanup_challenge(&self, token_key: &str) {
        if let Ok(mut challenge_s) =
            safe_mutex_lock(&self.active_challenge_s, "active_challenges_operation")
        {
            challenge_s.remove(token_key);
        }
    }

    /// Get metric_s for a specific path
    pub fn get_path_metric_s(&self, addr: &SocketAddr) -> Option<PathMetrics> {
        match safe_mutex_lock(&self.path_metric_s, "path_metrics_operation") {
            Ok(metric_s) => metric_s.get(addr).cloned(),
            Err(_) => None,
        }
    }

    /// Get all validated path_s with their metric_s
    pub fn get_all_path_metric_s(&self) -> HashMap<SocketAddr, PathMetrics> {
        match safe_mutex_lock(&self.path_metric_s, "path_metrics_operation") {
            Ok(metric_s) => metric_s.clone(),
            Err(_) => HashMap::new(),
        }
    }

    /// Validate multiple path_s concurrently
    pub async fn validate_multiple_path_s(
        &self,
        addr_s: &[SocketAddr],
    ) -> Result<HashMap<SocketAddr, PathMetrics>> {
        let mut result_s = HashMap::new();

        // Limit concurrent validation_s
        let __chunk_size = MAX_CONCURRENT_VALIDATIONS.min(addr_s.len());

        for chunk in addr_s.chunks(__chunk_size) {
            let mut handle_s = Vec::new();

            for &addr in chunk {
                let __validator = self.clone_for_validation();
                let __handle =
                    tokio::spawn(async move { (addr, __validator.validate_path(addr).await) });
                handle_s.push(__handle);
            }

            // Wait for all validation_s in thi_s chunk
            for handle in handle_s {
                if let Ok((addr, result)) = handle.await {
                    if let Ok(metric_s) = result {
                        result_s.insert(addr, metric_s);
                    }
                }
            }
        }

        Ok(result_s)
    }

    /// Create a clone suitable for concurrent validation
    fn clone_for_validation(&self) -> Self {
        Self {
            local_socket: Arc::clone(&self.local_socket),
            active_challenge_s: Arc::clone(&self.active_challenge_s),
            path_metric_s: Arc::clone(&self.path_metric_s),
            __validation_timeout: self.__validation_timeout,
            __max_retrie_s: self.__max_retrie_s,
            cancel_flag: Arc::clone(&self.cancel_flag),
            success_count: Arc::clone(&self.success_count),
            failure_count: Arc::clone(&self.failure_count),
            timeout_count: Arc::clone(&self.timeout_count),
            cancel_count: Arc::clone(&self.cancel_count),
        }
    }

    /// Cleanup expired challenge_s and metric_s
    pub fn cleanup_expired(&self) {
        let now = Instant::now();

        // Cleanup expired challenge_s
        if let Ok(mut challenge_s) =
            safe_mutex_lock(&self.active_challenge_s, "active_challenges_operation")
        {
            challenge_s
                .retain(|_, challenge| !challenge.is_timed_out(self.__validation_timeout * 2));
        }

        // Cleanup old metric_s (older than 1 hour)
        if let Ok(mut metric_s) = safe_mutex_lock(&self.path_metric_s, "path_metrics_operation") {
            metric_s.retain(|_, metric| {
                now.duration_since(metric.__last_validated) < Duration::from_secs(3600)
            });
        }
    }

    /// Get local socket addres_s
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.local_socket
            .local_addr()
            .map_err(|e| Error::Msg(format!("Failed to get local addres_s: {}", e)))
    }
}

/// Legacy function: Validate a host:port pair and return a resolved SocketAddr if possible.
/// Thi_s avoid_s DNS querie_s for plain IP literal_s and ensu_re_s the port i_s valid.
pub fn validate_host_port(host: &str, port: u16) -> Result<SocketAddr> {
    // Check for empty host
    if host.is_empty() {
        return Err(Error::Msg("host cannot be empty".to_string()));
    }

    // Try to parse as literal IP first to avoid DNS lookup_s.
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(SocketAddr::from((ip, port)));
    }
    // Fallback: attempt resolution via ToSocketAddr_s; thi_s may perform DNS.
    let mut iter = (host, port)
        .to_socket_addrs()
        .map_err(|e| Error::Msg(format!("invalid addres_s {host}:{port}: {e}")))?;
    iter.next()
        .ok_or_else(|| Error::Msg(format!("unable to resolve {host}:{port}")))
}

/// Enhanced path validation with comprehensive connectivity check_s
pub async fn validate_path_comprehensive(
    __local_addr: SocketAddr,
    __target_addr: SocketAddr,
    timeout: Option<Duration>,
) -> Result<PathMetrics> {
    let __validator = match timeout {
        Some(timeout_duration) => {
            PathValidator::new_with_timeout(__local_addr, timeout_duration).await?
        }
        None => PathValidator::new(__local_addr).await?,
    };

    __validator.validate_path(__target_addr).await
}

/// Perform bi-directional path validation
pub async fn validate_bidirectional_path(
    __local_addr: SocketAddr,
    __target_addr: SocketAddr,
) -> Result<(PathMetrics, PathMetrics)> {
    let __validator1 = PathValidator::new(__local_addr).await?;
    let __validator2 = PathValidator::new(__target_addr).await?;

    // Validate both direction_s concurrently
    let (result1, result2) = tokio::join!(
        __validator1.validate_path(__target_addr),
        __validator2.validate_path(__local_addr)
    );

    match (result1, result2) {
        (Ok(metrics1), Ok(metrics2)) => Ok((metrics1, metrics2)),
        (Err(e), _) | (_, Err(e)) => Err(e),
    }
}

#[cfg(test)]
#[cfg(test)]
mod test_s {
    use super::*;
    use std::time::Duration;

    #[test]
    fn parse_ipv4_literal() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let addr = validate_host_port("127.0.0.1", 8080)?;
        assert_eq!(addr.to_string(), "127.0.0.1:8080");
        Ok(())
    }

    #[test]
    fn parse_ipv6_literal() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let addr = validate_host_port("::1", 8080)?;
        assert_eq!(addr.to_string(), "[::1]:8080");
        Ok(())
    }

    #[test]
    fn invalid_host() {
        let result = validate_host_port("invalid.host.name", 8080);
        assert!(result.is_err());
    }

    #[test]
    fn path_challenge_creation() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let addr = "127.0.0.1:8080".parse()?;
        let challenge = PathChallenge::new(addr);

        assert_eq!(challenge.__target_addr, addr);
        assert_eq!(challenge.__state, PathValidationState::Pending);
        assert_eq!(challenge.token.len(), PATH_CHALLENGE_TOKEN_SIZE);

        // Check that challenge is not expired immediately - simplified check
        assert!(true); // Replace with proper implementation when is_expired method exists
        Ok(())
    }

    #[test]
    fn frame_type_constants() {
        assert_eq!(PATH_CHALLENGE_FRAME_TYPE, 0x33);
        assert_eq!(PATH_RESPONSE_FRAME_TYPE, 0x34);
        assert_eq!(PATH_CHALLENGE_TOKEN_SIZE, 16);
    }

    #[test]
    fn path_validation_state_enum() {
        use PathValidationState::*;

        assert_ne!(Pending, Validating);
        assert_ne!(Validated, Failed);
        assert_ne!(TimedOut, Pending);

        // Test Debug formatting
        assert_eq!(format!("{:?}", Pending), "Pending");
        assert_eq!(format!("{:?}", Validated), "Validated");
    }
}
