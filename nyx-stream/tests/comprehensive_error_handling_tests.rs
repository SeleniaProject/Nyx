//! Comprehensive tests for Comprehensive Error Handling
//!
//! This test suite validates the sophisticated error handling system including
//! automatic recovery, error classification, statistics tracking, and reporting.

use nyx_stream::{
    ErrorCategory, ErrorHandler, ErrorHandlingConfig, ErrorSeverity, IntoNyxError, NyxError,
    RecoveryStrategy,
};
use std::io;
use std::time::Duration;

/// Test basic error handler functionality
#[test]
fn test_error_handler_creation() {
    let config = ErrorHandlingConfig::default();
    let handler = ErrorHandler::new(config);

    let stats = handler.get_statistics();
    assert_eq!(stats.total_errors, 0);
    assert_eq!(stats.recovery_rate, 0.0);
}

/// Test error creation and properties
#[test]
fn test_error_creation() {
    let error = NyxError::new(
        "network_timeout",
        "Connection timed out after 30 seconds",
        ErrorSeverity::High,
        ErrorCategory::Network,
    )
    .with_recovery(RecoveryStrategy::Retry)
    .with_context("endpoint", "192.168.1.100:8080")
    .with_context("timeout", "30s")
    .with_source("network.rs:145");

    assert_eq!(error.error_type, "network_timeout");
    assert_eq!(error.message, "Connection timed out after 30 seconds");
    assert_eq!(error.severity, ErrorSeverity::High);
    assert_eq!(error.category, ErrorCategory::Network);
    assert_eq!(error.recovery, RecoveryStrategy::Retry);
    assert_eq!(error.count, 1);
    assert!(error.is_recoverable());

    // Check context
    assert_eq!(
        error.context.get("endpoint"),
        Some(&"192.168.1.100:8080".to_string())
    );
    assert_eq!(error.context.get("timeout"), Some(&"30s".to_string()));

    // Check source
    assert_eq!(error.source, Some("network.rs:145".to_string()));
}

/// Test error urgency scoring
#[test]
fn test_error_urgency_scoring() {
    let critical_crypto = NyxError::new(
        "key_exchange_failed",
        "Critical cryptographic failure",
        ErrorSeverity::Critical,
        ErrorCategory::Cryptographic,
    );

    let medium_network = NyxError::new(
        "connection_slow",
        "Network connection degraded",
        ErrorSeverity::Medium,
        ErrorCategory::Network,
    );

    let low_app = NyxError::new(
        "cache_miss",
        "Cache miss occurred",
        ErrorSeverity::Low,
        ErrorCategory::Application,
    );

    assert!(critical_crypto.urgency_score() > medium_network.urgency_score());
    assert!(medium_network.urgency_score() > low_app.urgency_score());

    // Test frequency impact
    let mut frequent_error = low_app.clone();
    for _ in 0..5 {
        frequent_error.increment();
    }

    assert!(frequent_error.urgency_score() > low_app.urgency_score());
}

/// Test error handling without recovery
#[tokio::test]
async fn test_error_handling_no_recovery() {
    let config = ErrorHandlingConfig {
        auto_recovery: false,
        ..Default::default()
    };
    let mut handler = ErrorHandler::new(config);

    let error = NyxError::new(
        "test_error",
        "Test error message",
        ErrorSeverity::Medium,
        ErrorCategory::Application,
    );

    let result = handler.handle_error(error.clone()).await;
    assert!(result.is_err());

    let stats = handler.get_statistics();
    assert_eq!(stats.total_errors, 1);
    assert!(stats.by_severity.contains_key(&ErrorSeverity::Medium));
    assert!(stats.by_category.contains_key(&ErrorCategory::Application));
}

/// Test error handling with retry recovery
#[tokio::test]
async fn test_error_handling_with_retry() {
    let config = ErrorHandlingConfig {
        auto_recovery: true,
        max_retries: 2,
        initial_retry_delay: Duration::from_millis(10),
        ..Default::default()
    };
    let mut handler = ErrorHandler::new(config);

    let error = NyxError::new(
        "retry_test",
        "Retryable error",
        ErrorSeverity::Medium,
        ErrorCategory::Network,
    )
    .with_recovery(RecoveryStrategy::Retry);

    let start_time = std::time::Instant::now();
    let _ = handler.handle_error(error).await;
    let elapsed = start_time.elapsed();

    // Should complete relatively quickly
    assert!(elapsed < Duration::from_secs(2));

    // Result can be either success or failure (probabilistic)
    let stats = handler.get_statistics();
    assert_eq!(stats.total_errors, 1);
}

/// Test error handling with fallback recovery
#[tokio::test]
async fn test_error_handling_with_fallback() {
    let config = ErrorHandlingConfig {
        auto_recovery: true,
        ..Default::default()
    };
    let mut handler = ErrorHandler::new(config);

    let error = NyxError::new(
        "fallback_test",
        "Error requiring fallback",
        ErrorSeverity::High,
        ErrorCategory::Protocol,
    )
    .with_recovery(RecoveryStrategy::Fallback);

    let _ = handler.handle_error(error).await;

    // Fallback has high success rate, but can still fail
    let stats = handler.get_statistics();
    assert_eq!(stats.total_errors, 1);
}

/// Test error statistics tracking
#[tokio::test]
async fn test_error_statistics() {
    let config = ErrorHandlingConfig::default();
    let mut handler = ErrorHandler::new(config);

    // Generate various types of errors
    let errors = vec![
        NyxError::new(
            "error1",
            "Error 1",
            ErrorSeverity::High,
            ErrorCategory::Network,
        )
        .with_recovery(RecoveryStrategy::Retry),
        NyxError::new(
            "error2",
            "Error 2",
            ErrorSeverity::Medium,
            ErrorCategory::Protocol,
        )
        .with_recovery(RecoveryStrategy::Fallback),
        NyxError::new(
            "error3",
            "Error 3",
            ErrorSeverity::Low,
            ErrorCategory::Application,
        )
        .with_recovery(RecoveryStrategy::Degrade),
        NyxError::new(
            "error1",
            "Error 1 again",
            ErrorSeverity::High,
            ErrorCategory::Network,
        )
        .with_recovery(RecoveryStrategy::Retry),
    ];

    for error in errors {
        let _ = handler.handle_error(error).await;
    }

    let stats = handler.get_statistics();
    assert_eq!(stats.total_errors, 4);

    // Check severity distribution
    assert!(stats.by_severity.get(&ErrorSeverity::High).unwrap_or(&0) >= &1);
    assert!(stats.by_severity.get(&ErrorSeverity::Medium).unwrap_or(&0) >= &1);
    assert!(stats.by_severity.get(&ErrorSeverity::Low).unwrap_or(&0) >= &1);

    // Check category distribution
    assert!(stats.by_category.get(&ErrorCategory::Network).unwrap_or(&0) >= &1);
    assert!(
        stats
            .by_category
            .get(&ErrorCategory::Protocol)
            .unwrap_or(&0)
            >= &1
    );
    assert!(
        stats
            .by_category
            .get(&ErrorCategory::Application)
            .unwrap_or(&0)
            >= &1
    );

    // Check frequent errors
    assert!(!stats.frequent_errors.is_empty());
}

/// Test error patterns analysis
#[tokio::test]
async fn test_error_patterns() {
    let config = ErrorHandlingConfig::default();
    let mut handler = ErrorHandler::new(config);

    // Create repeated errors to establish patterns
    for _i in 0..5 {
        let error = NyxError::new(
            "repeated_error",
            "Repeated error message",
            ErrorSeverity::Medium,
            ErrorCategory::Network,
        );
        let _ = handler.handle_error(error).await;
    }

    for _i in 0..3 {
        let error = NyxError::new(
            "occasional_error",
            "Occasional error message",
            ErrorSeverity::Low,
            ErrorCategory::Application,
        );
        let _ = handler.handle_error(error).await;
    }

    let patterns = handler.get_error_patterns();
    assert!(!patterns.is_empty());

    // Most frequent should be first
    assert!(patterns[0].1 >= patterns.get(1).map(|p| p.1).unwrap_or(0));
}

/// Test error report generation
#[tokio::test]
async fn test_error_report_generation() {
    let config = ErrorHandlingConfig::default();
    let mut handler = ErrorHandler::new(config);

    // Add some test errors
    let errors = vec![
        NyxError::new(
            "critical_error",
            "Critical failure",
            ErrorSeverity::Critical,
            ErrorCategory::Cryptographic,
        ),
        NyxError::new(
            "network_error",
            "Network issue",
            ErrorSeverity::High,
            ErrorCategory::Network,
        ),
        NyxError::new(
            "config_error",
            "Configuration problem",
            ErrorSeverity::Medium,
            ErrorCategory::Configuration,
        ),
    ];

    for error in errors {
        let _ = handler.handle_error(error).await;
    }

    let report = handler.generate_report();

    assert!(report.contains("Nyx Protocol Error Analysis Report"));
    assert!(report.contains("Total Errors:"));
    assert!(report.contains("Recovery Rate:"));
    assert!(report.contains("Errors by Severity:"));
    assert!(report.contains("Errors by Category:"));
    assert!(report.contains("Most Frequent Errors:"));
}

/// Test recent errors filtering
#[tokio::test]
async fn test_recent_errors_filtering() {
    let config = ErrorHandlingConfig::default();
    let mut handler = ErrorHandler::new(config);

    // Add errors of different severities
    let critical_error = NyxError::new(
        "critical_test",
        "Critical error",
        ErrorSeverity::Critical,
        ErrorCategory::System,
    );

    let high_error = NyxError::new(
        "high_test",
        "High severity error",
        ErrorSeverity::High,
        ErrorCategory::Network,
    );

    let medium_error = NyxError::new(
        "medium_test",
        "Medium severity error",
        ErrorSeverity::Medium,
        ErrorCategory::Application,
    );

    let _ = handler.handle_error(critical_error).await;
    let _ = handler.handle_error(high_error).await;
    let _ = handler.handle_error(medium_error).await;

    // Get recent critical errors
    let recent_critical = handler.get_recent_errors(ErrorSeverity::Critical, 10);
    assert_eq!(recent_critical.len(), 1);
    assert_eq!(recent_critical[0].error_type, "critical_test");

    // Get recent high errors
    let recent_high = handler.get_recent_errors(ErrorSeverity::High, 10);
    assert_eq!(recent_high.len(), 1);
    assert_eq!(recent_high[0].error_type, "high_test");
}

/// Test IntoNyxError trait
#[test]
fn test_into_nyx_error_trait() {
    let io_error = io::Error::new(io::ErrorKind::ConnectionRefused, "Connection refused");
    let nyx_error = io_error.into_nyx_error(ErrorCategory::Network);

    assert_eq!(nyx_error.error_type, "external_error");
    assert!(nyx_error.message.contains("Connection refused"));
    assert_eq!(nyx_error.category, ErrorCategory::Network);
    assert_eq!(nyx_error.severity, ErrorSeverity::Medium);
}

/// Test error handler history management
#[tokio::test]
async fn test_history_management() {
    let config = ErrorHandlingConfig {
        history_size: 5, // Small history for testing
        ..Default::default()
    };
    let mut handler = ErrorHandler::new(config);

    // Add more errors than history size
    for i in 0..10 {
        let error_type = format!("error_{i}");
        let error_message = format!("Error number {i}");
        let error = NyxError::new(
            &error_type,
            &error_message,
            ErrorSeverity::Low,
            ErrorCategory::Application,
        );
        let _ = handler.handle_error(error).await;
    }

    let stats = handler.get_statistics();
    assert_eq!(stats.total_errors, 5); // Should be limited by history size
}

/// Test error handler cleanup
#[tokio::test]
async fn test_error_handler_cleanup() {
    let config = ErrorHandlingConfig::default();
    let mut handler = ErrorHandler::new(config);

    // Add some errors
    for i in 0..3 {
        let error_type = format!("error_{i}");
        let error = NyxError::new(
            &error_type,
            &"Test error".to_string(),
            ErrorSeverity::Medium,
            ErrorCategory::Network,
        );
        let _ = handler.handle_error(error).await;
    }

    let stats_before = handler.get_statistics();
    assert!(stats_before.total_errors > 0);

    // Clear history
    handler.clear_history();

    let stats_after = handler.get_statistics();
    assert_eq!(stats_after.total_errors, 0);
}
