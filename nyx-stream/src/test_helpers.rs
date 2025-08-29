//! Test helper utilities for consolidating repeated patterns
//!
//! This module provides common test utilities to eliminate code duplication
//! across integration tests, particularly for result matching patterns.

use std::sync::Arc;
use tokio::time::{timeout, Duration};

/// Test helper for validating successful results with timeout
pub async fn assert_success_within<T, E>(
    future: impl std::future::Future<Output = Result<T, E>>,
    timeout_duration: Duration,
) -> T
where
    E: std::fmt::Debug,
{
    let result = timeout(timeout_duration, future)
        .await
        .expect("Operation timed out");

    match result {
        Ok(value) => value,
        Err(e) => panic!("Expected success but got error: {:?}", e),
    }
}

/// Test helper for validating expected errors
pub async fn assert_error_within<T, E>(
    future: impl std::future::Future<Output = Result<T, E>>,
    timeout_duration: Duration,
) -> E
where
    T: std::fmt::Debug,
{
    let result = timeout(timeout_duration, future)
        .await
        .expect("Operation timed out");

    match result {
        Ok(value) => panic!("Expected error but got success: {:?}", value),
        Err(error) => error,
    }
}

/// Common test configuration builder
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub timeout: Duration,
    pub retry_attempts: u32,
    pub buffer_size: usize,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            retry_attempts: 3,
            buffer_size: 8192,
        }
    }
}

impl TestConfig {
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_retries(mut self, attempts: u32) -> Self {
        self.retry_attempts = attempts;
        self
    }

    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }
}

/// Test helper for retry logic with exponential backoff
pub async fn retry_with_backoff<T, E, F, Fut>(
    mut operation: F,
    max_attempts: u32,
    initial_delay: Duration,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Debug,
{
    let mut attempt = 0;
    let mut delay = initial_delay;

    loop {
        attempt += 1;

        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt >= max_attempts => return Err(e),
            Err(_) => {
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(delay * 2, Duration::from_secs(60));
            }
        }
    }
}

/// Mock stream data generator for testing
pub fn generate_test_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// Validate stream data integrity
pub fn validate_data_integrity(original: &[u8], received: &[u8]) -> bool {
    original.len() == received.len() && original == received
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[tokio::test]
    async fn test_success_helper() {
        let result =
            assert_success_within(async { Ok::<i32, &str>(42) }, Duration::from_millis(100)).await;

        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_retry_with_backoff() {
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        let result = retry_with_backoff(
            move || {
                let count = counter_clone.fetch_add(1, Ordering::SeqCst);
                async move {
                    if count < 2 {
                        Err("not yet")
                    } else {
                        Ok("success")
                    }
                }
            },
            5,
            Duration::from_millis(10),
        )
        .await;

        assert_eq!(result, Ok("success"));
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn test_data_generation() {
        let data = generate_test_data(10);
        assert_eq!(data.len(), 10);
        assert_eq!(data[0], 0);
        assert_eq!(data[9], 9);
    }

    #[test]
    fn test_data_validation() {
        let original = vec![1, 2, 3, 4, 5];
        let same = vec![1, 2, 3, 4, 5];
        let different = vec![1, 2, 3, 4, 6];

        assert!(validate_data_integrity(&original, &same));
        assert!(!validate_data_integrity(&original, &different));
    }
}
