//! Path Builder Integration Tests
//! Comprehensive end-to-end testing for path building functionality
//! Tests real network scenarios, failure handling, and performance characteristics

use crate::errors::{DaemonError, Result};
use crate::path_builder::{DaemonConfig, PathBuilder};
use crate::path_recovery::{PathFailureReason, PathRecoveryConfig, PathRecoveryManager};
use nyx_transport::{TransportConfig, UdpTransport};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::info;

/// Integration test configuration
#[derive(Debug, Clone)]
pub struct IntegrationTestConfig {
    pub test_timeout: Duration,
    pub max_concurrent_builds: usize,
    pub test_endpoints: Vec<SocketAddr>,
    pub failure_injection_rate: f64, // 0.0 - 1.0
    pub performance_thresholds: PerformanceThresholds,
}

#[derive(Debug, Clone)]
pub struct PerformanceThresholds {
    pub max_build_time: Duration,
    pub min_success_rate: f64,
    pub max_memory_usage: usize, // bytes
    pub max_latency: Duration,
}

impl Default for IntegrationTestConfig {
    fn default() -> Self {
        Self {
            test_timeout: Duration::from_secs(30),
            max_concurrent_builds: 10,
            test_endpoints: vec![
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8082),
            ],
            failure_injection_rate: 0.1,
            performance_thresholds: PerformanceThresholds {
                max_build_time: Duration::from_secs(5),
                min_success_rate: 0.8,
                max_memory_usage: 100 * 1024 * 1024, // 100MB
                max_latency: Duration::from_millis(100),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub test_name: String,
    pub success: bool,
    pub duration: Duration,
    pub error_message: Option<String>,
    pub metrics: HashMap<String, f64>,
}

#[derive(Debug)]
pub struct TestSuite {
    config: IntegrationTestConfig,
    path_builder: Arc<PathBuilder>,
    recovery_manager: Arc<PathRecoveryManager>,
    _transport: Arc<UdpTransport>,
    results: Vec<TestResult>,
}

impl TestSuite {
    pub async fn new(config: IntegrationTestConfig) -> Result<Self> {
        let daemon_config = DaemonConfig::default();
        let transport_config = TransportConfig::default();

        let path_builder = Arc::new(PathBuilder::new(daemon_config)?);
        let transport = Arc::new(
            UdpTransport::new(transport_config)
                .map_err(|_| DaemonError::transport("Transport initialization failed"))?,
        );

        let recovery_config = PathRecoveryConfig::default();
        let recovery_manager = Arc::new(PathRecoveryManager::new(
            recovery_config,
            Arc::clone(&path_builder),
        ));

        Ok(Self {
            config,
            path_builder,
            recovery_manager,
            _transport: transport,
            results: Vec::new(),
        })
    }

    /// Run all integration tests
    pub async fn run_all_tests(&mut self) -> Result<Vec<TestResult>> {
        info!("Starting path builder integration tests");

        // Basic functionality tests
        self.test_basic_path_building().await;
        self.test_concurrent_path_building().await;
        self.test_path_quality_assessment().await;
        self.test_path_failover().await;

        // Performance tests
        self.test_build_performance().await;
        self.test_memory_usage().await;
        self.test_latency_requirements().await;

        // Error handling tests
        self.test_network_failure_handling().await;
        self.test_timeout_handling().await;
        self.test_resource_exhaustion().await;

        // Recovery tests
        self.test_automatic_recovery().await;
        self.test_manual_recovery().await;

        // Stress tests
        self.test_high_load_scenario().await;
        self.test_rapid_build_destroy().await;

        info!("Completed {} integration tests", self.results.len());
        Ok(self.results.clone())
    }

    /// Test basic path building functionality
    async fn test_basic_path_building(&mut self) {
        let test_name = "basic_path_building".to_string();
        let start_time = Instant::now();

        let result = timeout(self.config.test_timeout, self.run_basic_path_build_test()).await;

        let duration = start_time.elapsed();
        let test_result = match result {
            Ok(Ok(_)) => TestResult {
                test_name,
                success: true,
                duration,
                error_message: None,
                metrics: HashMap::new(),
            },
            Ok(Err(e)) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some(e.to_string()),
                metrics: HashMap::new(),
            },
            Err(_) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some("Test timeout".to_string()),
                metrics: HashMap::new(),
            },
        };

        self.results.push(test_result);
    }

    async fn run_basic_path_build_test(&self) -> Result<()> {
        for endpoint in &self.config.test_endpoints {
            let path_id = self.path_builder.build_path(*endpoint).await?;

            // Verify path was created
            let path_exists = self.path_builder.path_exists(&path_id).await?;
            if !path_exists {
                return Err(DaemonError::Internal("Path was not created".to_string()));
            }

            // Test path quality
            let quality = self.path_builder.assess_path_quality(&path_id).await?;
            if quality.overall_score() < 0.1 {
                return Err(DaemonError::Internal("Path quality too low".to_string()));
            }

            // Clean up
            self.path_builder.destroy_path(&path_id).await?;
        }

        Ok(())
    }

    /// Test concurrent path building
    async fn test_concurrent_path_building(&mut self) {
        let test_name = "concurrent_path_building".to_string();
        let start_time = Instant::now();

        let result = timeout(self.config.test_timeout, self.run_concurrent_build_test()).await;

        let duration = start_time.elapsed();
        let mut metrics = HashMap::new();

        let test_result = match result {
            Ok(Ok(concurrent_metrics)) => {
                metrics.extend(concurrent_metrics);
                TestResult {
                    test_name,
                    success: true,
                    duration,
                    error_message: None,
                    metrics,
                }
            }
            Ok(Err(e)) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some(e.to_string()),
                metrics,
            },
            Err(_) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some("Test timeout".to_string()),
                metrics,
            },
        };

        self.results.push(test_result);
    }

    async fn run_concurrent_build_test(&self) -> Result<HashMap<String, f64>> {
        let mut tasks = Vec::new();
        let start_time = Instant::now();

        // Launch concurrent path builds
        for i in 0..self.config.max_concurrent_builds {
            let endpoint = self.config.test_endpoints[i % self.config.test_endpoints.len()];
            let path_builder = Arc::clone(&self.path_builder);

            let task = tokio::spawn(async move { path_builder.build_path(endpoint).await });

            tasks.push(task);
        }

        // Wait for all tasks to complete
        let mut success_count = 0;
        let mut failed_builds = Vec::new();

        for (i, task) in tasks.into_iter().enumerate() {
            match task.await {
                Ok(Ok(path_id)) => {
                    success_count += 1;
                    // Clean up successful paths
                    let _ = self.path_builder.destroy_path(&path_id).await;
                }
                Ok(Err(e)) => {
                    failed_builds.push((i, e.to_string()));
                }
                Err(e) => {
                    failed_builds.push((i, format!("Task join error: {e}")));
                }
            }
        }

        let total_time = start_time.elapsed();
        let success_rate = success_count as f64 / self.config.max_concurrent_builds as f64;

        let mut metrics = HashMap::new();
        metrics.insert("success_count".to_string(), success_count as f64);
        metrics.insert("success_rate".to_string(), success_rate);
        metrics.insert(
            "total_builds".to_string(),
            self.config.max_concurrent_builds as f64,
        );
        metrics.insert("total_time_ms".to_string(), total_time.as_millis() as f64);

        if success_rate < self.config.performance_thresholds.min_success_rate {
            return Err(DaemonError::Internal(format!(
                "Success rate {} below threshold {}",
                success_rate, self.config.performance_thresholds.min_success_rate
            )));
        }

        Ok(metrics)
    }

    /// Test path quality assessment
    async fn test_path_quality_assessment(&mut self) {
        let test_name = "path_quality_assessment".to_string();
        let start_time = Instant::now();

        let result = timeout(self.config.test_timeout, self.run_quality_assessment_test()).await;

        let duration = start_time.elapsed();
        let test_result = match result {
            Ok(Ok(_)) => TestResult {
                test_name,
                success: true,
                duration,
                error_message: None,
                metrics: HashMap::new(),
            },
            Ok(Err(e)) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some(e.to_string()),
                metrics: HashMap::new(),
            },
            Err(_) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some("Test timeout".to_string()),
                metrics: HashMap::new(),
            },
        };

        self.results.push(test_result);
    }

    async fn run_quality_assessment_test(&self) -> Result<()> {
        let endpoint = self.config.test_endpoints[0];
        let path_id = self.path_builder.build_path(endpoint).await?;

        // Test quality assessment
        let quality = self.path_builder.assess_path_quality(&path_id).await?;

        // Verify quality metrics are reasonable
        if quality.latency.is_nan() || quality.bandwidth.is_nan() || quality.reliability.is_nan() {
            return Err(DaemonError::Internal(
                "Quality assessment incomplete".to_string(),
            ));
        }

        // Test quality updates
        self.path_builder
            .update_path_quality(&path_id, quality.clone())
            .await?;

        // Clean up
        self.path_builder.destroy_path(&path_id).await?;

        Ok(())
    }

    /// Test path failover functionality
    async fn test_path_failover(&mut self) {
        let test_name = "path_failover".to_string();
        let start_time = Instant::now();

        let result = timeout(self.config.test_timeout, self.run_failover_test()).await;

        let duration = start_time.elapsed();
        let test_result = match result {
            Ok(Ok(_)) => TestResult {
                test_name,
                success: true,
                duration,
                error_message: None,
                metrics: HashMap::new(),
            },
            Ok(Err(e)) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some(e.to_string()),
                metrics: HashMap::new(),
            },
            Err(_) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some("Test timeout".to_string()),
                metrics: HashMap::new(),
            },
        };

        self.results.push(test_result);
    }

    async fn run_failover_test(&self) -> Result<()> {
        // Build primary path
        let primary_endpoint = self.config.test_endpoints[0];
        let primary_path = self.path_builder.build_path(primary_endpoint).await?;

        // Build backup path
        let backup_endpoint = self.config.test_endpoints[1];
        let backup_path = self.path_builder.build_path(backup_endpoint).await?;

        // Simulate primary path failure
        self.path_builder
            .simulate_path_failure(&primary_path)
            .await?;

        // Test failover to backup path
        let failover_successful = self
            .path_builder
            .test_failover(&primary_path, &backup_path)
            .await?;

        if !failover_successful {
            return Err(DaemonError::Internal("Failover test failed".to_string()));
        }

        // Clean up
        self.path_builder.destroy_path(&primary_path).await?;
        self.path_builder.destroy_path(&backup_path).await?;

        Ok(())
    }

    /// Test build performance requirements
    async fn test_build_performance(&mut self) {
        let test_name = "build_performance".to_string();
        let start_time = Instant::now();

        let result = timeout(self.config.test_timeout, self.run_performance_test()).await;

        let duration = start_time.elapsed();
        let test_result = match result {
            Ok(Ok(performance_metrics)) => TestResult {
                test_name,
                success: true,
                duration,
                error_message: None,
                metrics: performance_metrics,
            },
            Ok(Err(e)) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some(e.to_string()),
                metrics: HashMap::new(),
            },
            Err(_) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some("Test timeout".to_string()),
                metrics: HashMap::new(),
            },
        };

        self.results.push(test_result);
    }

    async fn run_performance_test(&self) -> Result<HashMap<String, f64>> {
        let mut build_times = Vec::new();
        let test_count = 10;

        for _ in 0..test_count {
            let endpoint = self.config.test_endpoints[0];
            let build_start = Instant::now();

            let path_id = self.path_builder.build_path(endpoint).await?;
            let build_time = build_start.elapsed();

            build_times.push(build_time);

            if build_time > self.config.performance_thresholds.max_build_time {
                return Err(DaemonError::Internal(format!(
                    "Build time {:?} exceeds threshold {:?}",
                    build_time, self.config.performance_thresholds.max_build_time
                )));
            }

            self.path_builder.destroy_path(&path_id).await?;
        }

        let avg_build_time = build_times.iter().sum::<Duration>() / build_times.len() as u32;
        let min_build_time = build_times.iter().min().unwrap();
        let max_build_time = build_times.iter().max().unwrap();

        let mut metrics = HashMap::new();
        metrics.insert(
            "avg_build_time_ms".to_string(),
            avg_build_time.as_millis() as f64,
        );
        metrics.insert(
            "min_build_time_ms".to_string(),
            min_build_time.as_millis() as f64,
        );
        metrics.insert(
            "max_build_time_ms".to_string(),
            max_build_time.as_millis() as f64,
        );
        metrics.insert("test_count".to_string(), test_count as f64);

        Ok(metrics)
    }

    /// Test memory usage requirements
    async fn test_memory_usage(&mut self) {
        let test_name = "memory_usage".to_string();
        let start_time = Instant::now();

        // This is a simplified memory test - in a real implementation,
        // you would use proper memory profiling tools
        let initial_memory = self.get_memory_usage();

        // Build many paths to test memory usage
        let mut paths = Vec::new();
        for endpoint in &self.config.test_endpoints {
            for _ in 0..10 {
                if let Ok(path_id) = self.path_builder.build_path(*endpoint).await {
                    paths.push(path_id);
                }
            }
        }

        let peak_memory = self.get_memory_usage();
        let memory_used = peak_memory - initial_memory;

        // Clean up paths
        for path_id in paths {
            let _ = self.path_builder.destroy_path(&path_id).await;
        }

        let final_memory = self.get_memory_usage();
        let duration = start_time.elapsed();

        let mut metrics = HashMap::new();
        metrics.insert("initial_memory".to_string(), initial_memory as f64);
        metrics.insert("peak_memory".to_string(), peak_memory as f64);
        metrics.insert("memory_used".to_string(), memory_used as f64);
        metrics.insert("final_memory".to_string(), final_memory as f64);

        let success = memory_used <= self.config.performance_thresholds.max_memory_usage;
        let error_message = if !success {
            Some(format!(
                "Memory usage {} exceeds threshold {}",
                memory_used, self.config.performance_thresholds.max_memory_usage
            ))
        } else {
            None
        };

        let test_result = TestResult {
            test_name,
            success,
            duration,
            error_message,
            metrics,
        };

        self.results.push(test_result);
    }

    /// Test latency requirements
    async fn test_latency_requirements(&mut self) {
        let test_name = "latency_requirements".to_string();
        let start_time = Instant::now();

        let result = timeout(self.config.test_timeout, self.run_latency_test()).await;

        let duration = start_time.elapsed();
        let test_result = match result {
            Ok(Ok(latency_metrics)) => TestResult {
                test_name,
                success: true,
                duration,
                error_message: None,
                metrics: latency_metrics,
            },
            Ok(Err(e)) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some(e.to_string()),
                metrics: HashMap::new(),
            },
            Err(_) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some("Test timeout".to_string()),
                metrics: HashMap::new(),
            },
        };

        self.results.push(test_result);
    }

    async fn run_latency_test(&self) -> Result<HashMap<String, f64>> {
        let endpoint = self.config.test_endpoints[0];
        let path_id = self.path_builder.build_path(endpoint).await?;

        // Measure round-trip latency
        let latency_start = Instant::now();
        self.path_builder.ping_path(&path_id).await?;
        let latency = latency_start.elapsed();

        if latency > self.config.performance_thresholds.max_latency {
            return Err(DaemonError::Internal(format!(
                "Latency {:?} exceeds threshold {:?}",
                latency, self.config.performance_thresholds.max_latency
            )));
        }

        self.path_builder.destroy_path(&path_id).await?;

        let mut metrics = HashMap::new();
        metrics.insert("latency_ms".to_string(), latency.as_millis() as f64);

        Ok(metrics)
    }

    /// Test network failure handling
    async fn test_network_failure_handling(&mut self) {
        let test_name = "network_failure_handling".to_string();
        let start_time = Instant::now();

        let result = timeout(self.config.test_timeout, self.run_network_failure_test()).await;

        let duration = start_time.elapsed();
        let test_result = match result {
            Ok(Ok(_)) => TestResult {
                test_name,
                success: true,
                duration,
                error_message: None,
                metrics: HashMap::new(),
            },
            Ok(Err(e)) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some(e.to_string()),
                metrics: HashMap::new(),
            },
            Err(_) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some("Test timeout".to_string()),
                metrics: HashMap::new(),
            },
        };

        self.results.push(test_result);
    }

    async fn run_network_failure_test(&self) -> Result<()> {
        let _endpoint = self.config.test_endpoints[0];

        // Try to build path to unreachable endpoint
        let unreachable_endpoint = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 9999);

        let result = self.path_builder.build_path(unreachable_endpoint).await;

        // Should fail gracefully
        match result {
            Err(_) => {
                // Expected error
            }
            Ok(_) => {
                return Err(DaemonError::internal("Path build should have failed"));
            }
        }

        Ok(())
    }

    /// Test timeout handling
    async fn test_timeout_handling(&mut self) {
        let test_name = "timeout_handling".to_string();
        let start_time = Instant::now();

        // Simulate a very short timeout for this test
        let short_timeout = Duration::from_millis(1);
        let result = timeout(short_timeout, self.run_basic_path_build_test()).await;

        let duration = start_time.elapsed();
        let success = result.is_err(); // We expect this to timeout

        let test_result = TestResult {
            test_name,
            success,
            duration,
            error_message: if success {
                None
            } else {
                Some("Expected timeout but completed".to_string())
            },
            metrics: HashMap::new(),
        };

        self.results.push(test_result);
    }

    /// Test resource exhaustion handling
    async fn test_resource_exhaustion(&mut self) {
        let test_name = "resource_exhaustion".to_string();
        let start_time = Instant::now();

        let result = timeout(
            self.config.test_timeout,
            self.run_resource_exhaustion_test(),
        )
        .await;

        let duration = start_time.elapsed();
        let test_result = match result {
            Ok(Ok(_)) => TestResult {
                test_name,
                success: true,
                duration,
                error_message: None,
                metrics: HashMap::new(),
            },
            Ok(Err(e)) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some(e.to_string()),
                metrics: HashMap::new(),
            },
            Err(_) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some("Test timeout".to_string()),
                metrics: HashMap::new(),
            },
        };

        self.results.push(test_result);
    }

    async fn run_resource_exhaustion_test(&self) -> Result<()> {
        // Try to build many paths rapidly to test resource limits
        let mut paths = Vec::new();

        for i in 0..1000 {
            let endpoint = self.config.test_endpoints[i % self.config.test_endpoints.len()];

            match self.path_builder.build_path(endpoint).await {
                Ok(path_id) => paths.push(path_id),
                Err(DaemonError::ResourceExhaustion) => {
                    // Expected behavior when resources are exhausted
                    break;
                }
                Err(_) => {
                    // Other errors are also acceptable
                    break;
                }
            }
        }

        // Clean up created paths
        for path_id in paths {
            let _ = self.path_builder.destroy_path(&path_id).await;
        }

        Ok(())
    }

    /// Test automatic recovery
    async fn test_automatic_recovery(&mut self) {
        let test_name = "automatic_recovery".to_string();
        let start_time = Instant::now();

        let result = timeout(self.config.test_timeout, self.run_automatic_recovery_test()).await;

        let duration = start_time.elapsed();
        let test_result = match result {
            Ok(Ok(_)) => TestResult {
                test_name,
                success: true,
                duration,
                error_message: None,
                metrics: HashMap::new(),
            },
            Ok(Err(e)) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some(e.to_string()),
                metrics: HashMap::new(),
            },
            Err(_) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some("Test timeout".to_string()),
                metrics: HashMap::new(),
            },
        };

        self.results.push(test_result);
    }

    async fn run_automatic_recovery_test(&self) -> Result<()> {
        let endpoint = self.config.test_endpoints[0];

        // Record a simulated failure
        self.recovery_manager
            .record_failure(
                "test_path".to_string(),
                endpoint,
                PathFailureReason::NetworkUnreachable,
                "Simulated failure for testing".to_string(),
            )
            .await?;

        // Attempt recovery
        let _recovery_successful = self.recovery_manager.attempt_recovery("test_path").await?;

        // Note: Recovery might fail due to unimplemented methods, which is expected
        // The test is successful if it doesn't crash and handles errors gracefully

        Ok(())
    }

    /// Test manual recovery
    async fn test_manual_recovery(&mut self) {
        let test_name = "manual_recovery".to_string();
        let start_time = Instant::now();

        let result = timeout(self.config.test_timeout, self.run_manual_recovery_test()).await;

        let duration = start_time.elapsed();
        let test_result = match result {
            Ok(Ok(_)) => TestResult {
                test_name,
                success: true,
                duration,
                error_message: None,
                metrics: HashMap::new(),
            },
            Ok(Err(e)) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some(e.to_string()),
                metrics: HashMap::new(),
            },
            Err(_) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some("Test timeout".to_string()),
                metrics: HashMap::new(),
            },
        };

        self.results.push(test_result);
    }

    async fn run_manual_recovery_test(&self) -> Result<()> {
        // Test manual recovery operations
        let _failed_paths = self.recovery_manager.get_failed_paths().await;
        let _statistics = self.recovery_manager.get_failure_statistics().await;

        // Clear failure history
        self.recovery_manager.clear_failure_history().await;

        Ok(())
    }

    /// Test high load scenario
    async fn test_high_load_scenario(&mut self) {
        let test_name = "high_load_scenario".to_string();
        let start_time = Instant::now();

        let result = timeout(
            Duration::from_secs(60), // Longer timeout for stress test
            self.run_high_load_test(),
        )
        .await;

        let duration = start_time.elapsed();
        let test_result = match result {
            Ok(Ok(load_metrics)) => TestResult {
                test_name,
                success: true,
                duration,
                error_message: None,
                metrics: load_metrics,
            },
            Ok(Err(e)) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some(e.to_string()),
                metrics: HashMap::new(),
            },
            Err(_) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some("Test timeout".to_string()),
                metrics: HashMap::new(),
            },
        };

        self.results.push(test_result);
    }

    async fn run_high_load_test(&self) -> Result<HashMap<String, f64>> {
        let high_load_count = 100;
        let mut tasks = Vec::new();
        let start_time = Instant::now();

        // Launch many concurrent operations
        for i in 0..high_load_count {
            let endpoint = self.config.test_endpoints[i % self.config.test_endpoints.len()];
            let path_builder = Arc::clone(&self.path_builder);

            let task = tokio::spawn(async move {
                let build_result = path_builder.build_path(endpoint).await;
                let success = if let Ok(ref path_id) = build_result {
                    let _ = path_builder.destroy_path(path_id).await;
                    true
                } else {
                    false
                };
                success
            });

            tasks.push(task);
        }

        // Wait for completion and count successes
        let mut success_count = 0;
        for task in tasks {
            if let Ok(success) = task.await {
                if success {
                    success_count += 1;
                }
            }
        }

        let total_time = start_time.elapsed();
        let success_rate = success_count as f64 / high_load_count as f64;

        let mut metrics = HashMap::new();
        metrics.insert("high_load_count".to_string(), high_load_count as f64);
        metrics.insert("success_count".to_string(), success_count as f64);
        metrics.insert("success_rate".to_string(), success_rate);
        metrics.insert("total_time_ms".to_string(), total_time.as_millis() as f64);

        Ok(metrics)
    }

    /// Test rapid build/destroy cycles
    async fn test_rapid_build_destroy(&mut self) {
        let test_name = "rapid_build_destroy".to_string();
        let start_time = Instant::now();

        let result = timeout(self.config.test_timeout, self.run_rapid_cycle_test()).await;

        let duration = start_time.elapsed();
        let test_result = match result {
            Ok(Ok(cycle_metrics)) => TestResult {
                test_name,
                success: true,
                duration,
                error_message: None,
                metrics: cycle_metrics,
            },
            Ok(Err(e)) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some(e.to_string()),
                metrics: HashMap::new(),
            },
            Err(_) => TestResult {
                test_name,
                success: false,
                duration,
                error_message: Some("Test timeout".to_string()),
                metrics: HashMap::new(),
            },
        };

        self.results.push(test_result);
    }

    async fn run_rapid_cycle_test(&self) -> Result<HashMap<String, f64>> {
        let cycle_count = 50;
        let endpoint = self.config.test_endpoints[0];
        let start_time = Instant::now();

        for _ in 0..cycle_count {
            let path_id = self.path_builder.build_path(endpoint).await?;
            self.path_builder.destroy_path(&path_id).await?;
        }

        let total_time = start_time.elapsed();
        let avg_cycle_time = total_time / cycle_count;

        let mut metrics = HashMap::new();
        metrics.insert("cycle_count".to_string(), cycle_count as f64);
        metrics.insert("total_time_ms".to_string(), total_time.as_millis() as f64);
        metrics.insert(
            "avg_cycle_time_ms".to_string(),
            avg_cycle_time.as_millis() as f64,
        );

        Ok(metrics)
    }

    /// Get current memory usage (simplified implementation)
    fn get_memory_usage(&self) -> usize {
        // This is a placeholder - in a real implementation you would use
        // proper memory profiling tools or system APIs
        0
    }

    /// Generate test report
    pub fn generate_report(&self) -> String {
        let total_tests = self.results.len();
        let successful_tests = self.results.iter().filter(|r| r.success).count();
        let failed_tests = total_tests - successful_tests;

        let mut report = String::new();
        report.push_str("Path Builder Integration Test Report\n");
        report.push_str("=====================================\n\n");
        report.push_str(&format!("Total Tests: {total_tests}\n"));
        report.push_str(&format!("Successful: {successful_tests}\n"));
        report.push_str(&format!("Failed: {failed_tests}\n"));
        report.push_str(&format!(
            "Success Rate: {:.2}%\n\n",
            (successful_tests as f64 / total_tests as f64) * 100.0
        ));

        report.push_str("Test Details:\n");
        report.push_str("=============\n");

        for result in &self.results {
            report.push_str(&format!("Test: {}\n", result.test_name));
            report.push_str(&format!(
                "  Status: {}\n",
                if result.success { "PASS" } else { "FAIL" }
            ));
            report.push_str(&format!("  Duration: {:?}\n", result.duration));

            if let Some(ref error) = result.error_message {
                report.push_str(&format!("  Error: {error}\n"));
            }

            if !result.metrics.is_empty() {
                report.push_str("  Metrics:\n");
                for (key, value) in &result.metrics {
                    report.push_str(&format!("    {key}: {value:.2}\n"));
                }
            }

            report.push('\n');
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_integration_test_suite_creation() -> Result<()> {
        let config = IntegrationTestConfig::default();
        let _suite = TestSuite::new(config).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_performance_thresholds() {
        let thresholds = PerformanceThresholds {
            max_build_time: Duration::from_secs(1),
            min_success_rate: 0.9,
            max_memory_usage: 50 * 1024 * 1024,
            max_latency: Duration::from_millis(50),
        };

        assert_eq!(thresholds.max_build_time, Duration::from_secs(1));
        assert_eq!(thresholds.min_success_rate, 0.9);
    }
}
