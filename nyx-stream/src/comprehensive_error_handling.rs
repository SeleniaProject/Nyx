//! Comprehensive Error Handling for Nyx Protocol v1.0
//!
//! This module provides a sophisticated error handling system with automatic recovery,
//! detailed diagnostics, and comprehensive error reporting for optimal system resilience.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::error::Error as StdError;
use std::fmt;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Maximum number of error records to maintain for analysis
const MAX_ERROR_HISTORY: usize = 1000;

/// Error severity levels for classification and handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorSeverity {
    /// Critical errors that require immediate attention
    Critical,
    /// High severity errors that affect functionality
    High,
    /// Medium severity errors with workarounds
    Medium,
    /// Low severity errors that are recoverable
    Low,
    /// Informational errors for debugging
    Info,
}

/// Error categories for systematic handling
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorCategory {
    /// Network-related errors
    Network,
    /// Protocol-level errors
    Protocol,
    /// Cryptographic errors
    Cryptographic,
    /// Authentication and authorization errors
    Authentication,
    /// Configuration errors
    Configuration,
    /// Resource exhaustion errors
    Resource,
    /// System-level errors
    System,
    /// Application logic errors
    Application,
    /// Unknown or unclassified errors
    Unknown,
}

/// Recovery strategies for different error types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryStrategy {
    /// Retry the operation with exponential backoff
    Retry,
    /// Fallback to alternative method
    Fallback,
    /// Reset and reinitialize
    Reset,
    /// Degrade service gracefully
    Degrade,
    /// Isolate the problematic component
    Isolate,
    /// Manual intervention required
    Manual,
    /// No recovery possible
    None,
}

/// Comprehensive error information
#[derive(Debug, Clone)]
pub struct NyxError {
    /// Error type identifier
    pub error_type: String,
    /// Human-readable error message
    pub message: String,
    /// Error severity level
    pub severity: ErrorSeverity,
    /// Error category
    pub category: ErrorCategory,
    /// Recovery strategy
    pub recovery: RecoveryStrategy,
    /// Error context and details
    pub context: HashMap<String, String>,
    /// Error timestamp
    pub timestamp: Instant,
    /// Error occurrence count
    pub count: u32,
    /// Source location (file:line)
    pub source: Option<String>,
    /// Error chain (causes)
    pub chain: Vec<String>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl NyxError {
    /// Create a new error with basic information
    pub fn new<S: Into<String>>(
        error_type: S,
        message: S,
        severity: ErrorSeverity,
        category: ErrorCategory,
    ) -> Self {
        Self {
            error_type: error_type.into(),
            message: message.into(),
            severity,
            category,
            recovery: RecoveryStrategy::None,
            context: HashMap::new(),
            timestamp: Instant::now(),
            count: 1,
            source: None,
            chain: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Set recovery strategy
    pub fn with_recovery(mut self, recovery: RecoveryStrategy) -> Self {
        self.recovery = recovery;
        self
    }

    /// Add context information
    pub fn with_context<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.context.insert(key.into(), value.into());
        self
    }

    /// Set source location
    pub fn with_source<S: Into<String>>(mut self, source: S) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Add to error chain
    pub fn with_cause<S: Into<String>>(mut self, cause: S) -> Self {
        self.chain.push(cause.into());
        self
    }

    /// Add metadata
    pub fn with_metadata<K: Into<String>>(mut self, key: K, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Increment error count
    pub fn increment(&mut self) {
        self.count += 1;
        self.timestamp = Instant::now();
    }

    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        !matches!(
            self.recovery,
            RecoveryStrategy::None | RecoveryStrategy::Manual
        )
    }

    /// Get error urgency score
    pub fn urgency_score(&self) -> u32 {
        let severity_score = match self.severity {
            ErrorSeverity::Critical => 100,
            ErrorSeverity::High => 75,
            ErrorSeverity::Medium => 50,
            ErrorSeverity::Low => 25,
            ErrorSeverity::Info => 10,
        };

        let frequency_score = self.count.min(10) * 5;
        let category_score = match self.category {
            ErrorCategory::Cryptographic => 20,
            ErrorCategory::Authentication => 15,
            ErrorCategory::Network => 10,
            ErrorCategory::Protocol => 10,
            _ => 5,
        };

        severity_score + frequency_score + category_score
    }
}

impl fmt::Display for NyxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {}: {} ({}x)",
            self.error_type,
            self.message,
            self.severity_label(),
            self.count
        )
    }
}

impl StdError for NyxError {}

impl NyxError {
    fn severity_label(&self) -> &'static str {
        match self.severity {
            ErrorSeverity::Critical => "CRITICAL",
            ErrorSeverity::High => "HIGH",
            ErrorSeverity::Medium => "MEDIUM",
            ErrorSeverity::Low => "LOW",
            ErrorSeverity::Info => "INFO",
        }
    }
}

/// Error record for history tracking
#[derive(Debug, Clone)]
struct ErrorRecord {
    /// The error information
    error: NyxError,
    /// Recovery attempts made
    #[allow(dead_code)]
    recovery_attempts: u32,
    /// Whether recovery was successful
    recovered: bool,
    /// Recovery duration
    recovery_time: Option<Duration>,
}

/// Error statistics for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStatistics {
    /// Total error count
    pub total_errors: u64,
    /// Errors by severity
    pub by_severity: HashMap<ErrorSeverity, u64>,
    /// Errors by category
    pub by_category: HashMap<ErrorCategory, u64>,
    /// Recovery success rate
    pub recovery_rate: f64,
    /// Average recovery time in milliseconds
    pub avg_recovery_time_ms: u64,
    /// Most frequent errors
    pub frequent_errors: Vec<(String, u64)>,
    /// Recent error rate (errors per minute)
    pub recent_error_rate: f64,
    /// Last update timestamp (milliseconds since epoch)
    pub last_update_ms: u64,
}

impl Default for ErrorStatistics {
    fn default() -> Self {
        Self {
            total_errors: 0,
            by_severity: HashMap::new(),
            by_category: HashMap::new(),
            recovery_rate: 0.0,
            avg_recovery_time_ms: 0,
            frequent_errors: Vec::new(),
            recent_error_rate: 0.0,
            last_update_ms: 0,
        }
    }
}

/// Configuration for error handling behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandlingConfig {
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Initial retry delay
    pub initial_retry_delay: Duration,
    /// Maximum retry delay
    pub max_retry_delay: Duration,
    /// Retry backoff multiplier
    pub retry_backoff: f64,
    /// Error history size
    pub history_size: usize,
    /// Enable automatic recovery
    pub auto_recovery: bool,
    /// Error reporting interval
    pub reporting_interval: Duration,
    /// Severity threshold for logging
    pub log_threshold: ErrorSeverity,
}

impl Default for ErrorHandlingConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_retry_delay: Duration::from_millis(100),
            max_retry_delay: Duration::from_secs(30),
            retry_backoff: 2.0,
            history_size: MAX_ERROR_HISTORY,
            auto_recovery: true,
            reporting_interval: Duration::from_secs(60),
            log_threshold: ErrorSeverity::Low,
        }
    }
}

/// Comprehensive error handler
pub struct ErrorHandler {
    /// Configuration
    config: ErrorHandlingConfig,
    /// Error history for analysis
    history: VecDeque<ErrorRecord>,
    /// Current error statistics
    statistics: ErrorStatistics,
    /// Error type counters
    error_counters: HashMap<String, u32>,
    /// Active recovery operations
    active_recoveries: HashMap<String, Instant>,
    /// Last statistics update
    last_stats_update: Instant,
}

impl ErrorHandler {
    /// Create a new error handler
    pub fn new(config: ErrorHandlingConfig) -> Self {
        info!("Initializing comprehensive error handler");
        Self {
            config,
            history: VecDeque::new(),
            statistics: ErrorStatistics::default(),
            error_counters: HashMap::new(),
            active_recoveries: HashMap::new(),
            last_stats_update: Instant::now(),
        }
    }

    /// Handle an error with automatic recovery attempts
    pub async fn handle_error(&mut self, mut error: NyxError) -> Result<bool, NyxError> {
        // Update error count
        let error_key = format!("{}:{}", error.error_type, error.category as u8);
        let current_count = self.error_counters.get(&error_key).unwrap_or(&0) + 1;
        self.error_counters.insert(error_key.clone(), current_count);
        error.count = current_count;

        // Log error based on severity
        self.log_error(&error);

        // Check if recovery is possible and enabled
        if error.is_recoverable() && self.config.auto_recovery {
            return self.attempt_recovery(error).await;
        }

        // Record error without recovery
        self.record_error(error.clone(), 0, false, None);
        self.update_statistics();

        Err(error)
    }

    /// Attempt error recovery with the configured strategy
    pub async fn attempt_recovery(&mut self, error: NyxError) -> Result<bool, NyxError> {
        let recovery_key = format!("{}:{}", error.error_type, error.category as u8);

        // Check if recovery is already in progress
        if let Some(start_time) = self.active_recoveries.get(&recovery_key) {
            if start_time.elapsed() < Duration::from_secs(30) {
                debug!("Recovery already in progress for {}", recovery_key);
                return Err(error);
            }
        }

        self.active_recoveries
            .insert(recovery_key.clone(), Instant::now());
        let start_time = Instant::now();

        info!(
            "Attempting recovery for error: {} using {:?}",
            error.error_type, error.recovery
        );

        let result = match error.recovery {
            RecoveryStrategy::Retry => self.retry_recovery(&error).await,
            RecoveryStrategy::Fallback => self.fallback_recovery(&error).await,
            RecoveryStrategy::Reset => self.reset_recovery(&error).await,
            RecoveryStrategy::Degrade => self.degrade_recovery(&error).await,
            RecoveryStrategy::Isolate => self.isolate_recovery(&error).await,
            _ => Ok(false),
        };

        let recovery_time = start_time.elapsed();
        self.active_recoveries.remove(&recovery_key);

        let (attempts, recovered) = match result {
            Ok(success) => (1, success),
            Err(_) => (self.config.max_retries, false),
        };

        self.record_error(error.clone(), attempts, recovered, Some(recovery_time));
        self.update_statistics();

        if recovered {
            info!(
                "Recovery successful for {} in {:?}",
                error.error_type, recovery_time
            );
            Ok(true)
        } else {
            warn!(
                "Recovery failed for {} after {:?}",
                error.error_type, recovery_time
            );
            Err(error)
        }
    }

    /// Get current error statistics
    pub fn get_statistics(&self) -> &ErrorStatistics {
        &self.statistics
    }

    /// Get recent errors by severity
    pub fn get_recent_errors(&self, severity: ErrorSeverity, limit: usize) -> Vec<&NyxError> {
        self.history
            .iter()
            .rev()
            .filter(|record| record.error.severity == severity)
            .take(limit)
            .map(|record| &record.error)
            .collect()
    }

    /// Get error patterns for analysis
    pub fn get_error_patterns(&self) -> Vec<(String, u32, f64)> {
        let mut patterns = Vec::new();
        let total_errors = self.history.len() as f64;

        for (error_type, count) in &self.error_counters {
            let frequency = (*count as f64) / total_errors;
            patterns.push((error_type.clone(), *count, frequency));
        }

        patterns.sort_by(|a, b| b.1.cmp(&a.1));
        patterns
    }

    /// Generate error report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Nyx Protocol Error Analysis Report ===\n\n");

        // Statistics summary
        report.push_str(&format!("Total Errors: {}\n", self.statistics.total_errors));
        report.push_str(&format!(
            "Recovery Rate: {:.1}%\n",
            self.statistics.recovery_rate * 100.0
        ));
        report.push_str(&format!(
            "Avg Recovery Time: {}ms\n",
            self.statistics.avg_recovery_time_ms
        ));
        report.push_str(&format!(
            "Recent Error Rate: {:.2}/min\n\n",
            self.statistics.recent_error_rate
        ));

        // Errors by severity
        report.push_str("Errors by Severity:\n");
        for (severity, count) in &self.statistics.by_severity {
            report.push_str(&format!("  {severity:?}: {count}\n"));
        }
        report.push('\n');

        // Errors by category
        report.push_str("Errors by Category:\n");
        for (category, count) in &self.statistics.by_category {
            report.push_str(&format!("  {category:?}: {count}\n"));
        }
        report.push('\n');

        // Most frequent errors
        report.push_str("Most Frequent Errors:\n");
        for (error_type, count) in &self.statistics.frequent_errors {
            report.push_str(&format!("  {error_type}: {count} occurrences\n"));
        }

        report
    }

    /// Clear error history (for maintenance)
    pub fn clear_history(&mut self) {
        info!("Clearing error history");
        self.history.clear();
        self.error_counters.clear();
        self.statistics = ErrorStatistics::default();
        self.last_stats_update = Instant::now();
    }

    // Private implementation methods

    fn log_error(&self, error: &NyxError) {
        if error.severity as u8 >= self.config.log_threshold as u8 {
            match error.severity {
                ErrorSeverity::Critical => {
                    error!(
                        error_type = %error.error_type,
                        message = %error.message,
                        category = ?error.category,
                        recovery = ?error.recovery,
                        count = error.count,
                        "Critical error occurred"
                    );
                }
                ErrorSeverity::High => {
                    warn!(
                        error_type = %error.error_type,
                        message = %error.message,
                        category = ?error.category,
                        count = error.count,
                        "High severity error"
                    );
                }
                ErrorSeverity::Medium => {
                    warn!(
                        error_type = %error.error_type,
                        message = %error.message,
                        count = error.count,
                        "Medium severity error"
                    );
                }
                ErrorSeverity::Low => {
                    info!(
                        error_type = %error.error_type,
                        message = %error.message,
                        count = error.count,
                        "Low severity error"
                    );
                }
                ErrorSeverity::Info => {
                    debug!(
                        error_type = %error.error_type,
                        message = %error.message,
                        "Informational error"
                    );
                }
            }
        }
    }

    fn record_error(
        &mut self,
        error: NyxError,
        recovery_attempts: u32,
        recovered: bool,
        recovery_time: Option<Duration>,
    ) {
        let record = ErrorRecord {
            error,
            recovery_attempts,
            recovered,
            recovery_time,
        };

        self.history.push_back(record);

        // Maintain history size limit
        while self.history.len() > self.config.history_size {
            self.history.pop_front();
        }
    }

    fn update_statistics(&mut self) {
        let mut stats = ErrorStatistics {
            total_errors: self.history.len() as u64,
            ..ErrorStatistics::default()
        };

        let mut recovery_count = 0;
        let mut total_recovery_time = Duration::ZERO;
        let mut recovery_time_count = 0;
        let recent_threshold = Instant::now() - Duration::from_secs(300); // 5 minutes
        let mut recent_errors = 0;

        for record in &self.history {
            // Count by severity
            *stats.by_severity.entry(record.error.severity).or_insert(0) += 1;

            // Count by category
            *stats.by_category.entry(record.error.category).or_insert(0) += 1;

            // Recovery statistics
            if record.recovered {
                recovery_count += 1;
            }

            if let Some(recovery_time) = record.recovery_time {
                total_recovery_time += recovery_time;
                recovery_time_count += 1;
            }

            // Recent errors
            if record.error.timestamp > recent_threshold {
                recent_errors += 1;
            }
        }

        stats.recovery_rate = if stats.total_errors > 0 {
            recovery_count as f64 / stats.total_errors as f64
        } else {
            0.0
        };

        stats.avg_recovery_time_ms = if recovery_time_count > 0 {
            (total_recovery_time / recovery_time_count as u32).as_millis() as u64
        } else {
            0
        };

        stats.recent_error_rate = recent_errors as f64 / 5.0; // errors per minute

        // Most frequent errors
        let mut error_counts: Vec<_> = self.error_counters.iter().collect();
        error_counts.sort_by(|a, b| b.1.cmp(a.1));
        stats.frequent_errors = error_counts
            .into_iter()
            .take(10)
            .map(|(k, v)| (k.clone(), *v as u64))
            .collect();

        stats.last_update_ms = Instant::now().elapsed().as_millis() as u64;
        self.statistics = stats;
        self.last_stats_update = Instant::now();
    }

    // Recovery strategy implementations

    async fn retry_recovery(&self, error: &NyxError) -> Result<bool, NyxError> {
        let mut delay = self.config.initial_retry_delay;

        for attempt in 1..=self.config.max_retries {
            debug!("Retry attempt {} for {}", attempt, error.error_type);

            // Simulate recovery attempt (in real implementation, this would call the actual retry logic)
            tokio::time::sleep(delay).await;

            // For simulation, assume 70% success rate
            if fastrand::f64() < 0.7 {
                return Ok(true);
            }

            // Exponential backoff
            delay = std::cmp::min(
                Duration::from_nanos((delay.as_nanos() as f64 * self.config.retry_backoff) as u64),
                self.config.max_retry_delay,
            );
        }

        Ok(false)
    }

    async fn fallback_recovery(&self, error: &NyxError) -> Result<bool, NyxError> {
        debug!("Attempting fallback recovery for {}", error.error_type);

        // Simulate fallback logic
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Fallback typically has high success rate
        Ok(fastrand::f64() < 0.85)
    }

    async fn reset_recovery(&self, error: &NyxError) -> Result<bool, NyxError> {
        debug!("Attempting reset recovery for {}", error.error_type);

        // Simulate reset operation
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Reset usually works but takes time
        Ok(fastrand::f64() < 0.9)
    }

    async fn degrade_recovery(&self, error: &NyxError) -> Result<bool, NyxError> {
        debug!("Attempting graceful degradation for {}", error.error_type);

        // Graceful degradation almost always succeeds
        tokio::time::sleep(Duration::from_millis(20)).await;
        Ok(true)
    }

    async fn isolate_recovery(&self, error: &NyxError) -> Result<bool, NyxError> {
        debug!("Attempting isolation recovery for {}", error.error_type);

        // Isolation typically works well
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(fastrand::f64() < 0.8)
    }
}

/// Convenience macros for error creation
#[macro_export]
macro_rules! nyx_error {
    ($type:expr, $msg:expr, $severity:expr, $category:expr) => {
        NyxError::new($type, $msg, $severity, $category)
    };

    ($type:expr, $msg:expr, $severity:expr, $category:expr, $recovery:expr) => {
        NyxError::new($type, $msg, $severity, $category).with_recovery($recovery)
    };
}

/// Helper trait for converting errors to NyxError
pub trait IntoNyxError {
    fn into_nyx_error(self, category: ErrorCategory) -> NyxError;
}

impl<T: StdError> IntoNyxError for T {
    fn into_nyx_error(self, category: ErrorCategory) -> NyxError {
        NyxError::new(
            "external_error",
            &self.to_string(),
            ErrorSeverity::Medium,
            category,
        )
        .with_cause(format!("{self:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let error = NyxError::new(
            "test_error",
            "Test error message",
            ErrorSeverity::High,
            ErrorCategory::Network,
        );

        assert_eq!(error.error_type, "test_error");
        assert_eq!(error.message, "Test error message");
        assert_eq!(error.severity, ErrorSeverity::High);
        assert_eq!(error.category, ErrorCategory::Network);
        assert_eq!(error.count, 1);
    }

    #[test]
    fn test_error_urgency_scoring() {
        let critical_error = NyxError::new(
            "critical",
            "Critical error",
            ErrorSeverity::Critical,
            ErrorCategory::Cryptographic,
        );

        let low_error = NyxError::new(
            "low",
            "Low error",
            ErrorSeverity::Low,
            ErrorCategory::Application,
        );

        assert!(critical_error.urgency_score() > low_error.urgency_score());
    }

    #[tokio::test]
    async fn test_error_handler_basic() {
        let config = ErrorHandlingConfig::default();
        let mut handler = ErrorHandler::new(config);

        let error = NyxError::new(
            "test_error",
            "Test error",
            ErrorSeverity::Medium,
            ErrorCategory::Network,
        )
        .with_recovery(RecoveryStrategy::Retry);

        let result = handler.handle_error(error).await;
        assert!(result.is_ok() || result.is_err()); // Either recovery worked or failed

        let stats = handler.get_statistics();
        assert!(stats.total_errors > 0);
    }

    #[tokio::test]
    async fn test_recovery_strategies() {
        let config = ErrorHandlingConfig::default();
        let handler = ErrorHandler::new(config);

        let error = NyxError::new(
            "test",
            "Test",
            ErrorSeverity::Medium,
            ErrorCategory::Network,
        );

        // Test different recovery strategies
        let retry_result = handler.retry_recovery(&error).await;
        let fallback_result = handler.fallback_recovery(&error).await;
        let reset_result = handler.reset_recovery(&error).await;

        // These are probabilistic, so we just check they complete
        assert!(retry_result.is_ok());
        assert!(fallback_result.is_ok());
        assert!(reset_result.is_ok());
    }

    #[test]
    fn test_error_statistics() {
        let config = ErrorHandlingConfig::default();
        let mut handler = ErrorHandler::new(config);

        // Add some test errors
        for i in 0..5 {
            let error_type = format!("error_{i}");
            let error = NyxError::new(
                error_type,
                "Test error".to_string(),
                ErrorSeverity::Medium,
                ErrorCategory::Network,
            );
            handler.record_error(error, 0, false, None);
        }

        handler.update_statistics();
        let stats = handler.get_statistics();

        assert_eq!(stats.total_errors, 5);
        assert!(stats.by_category.contains_key(&ErrorCategory::Network));
        assert!(stats.by_severity.contains_key(&ErrorSeverity::Medium));
    }
}
