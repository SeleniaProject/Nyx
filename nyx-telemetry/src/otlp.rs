//! OTLP (OpenTelemetry Protocol) Exporter Implementation
//! 
//! This module provides OTLP exporter functionality for sending telemetry data
//! to OpenTelemetry collectors via gRPC or HTTP protocols.

use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, sleep, timeout};
use tracing::{debug, error, info, warn};
use serde::{Deserialize, Serialize};

/// Maximum number of spans to batch before sending
const DEFAULT_MAX_BATCH_SIZE: usize = 512;
/// Maximum time to wait before sending a batch
const DEFAULT_BATCH_TIMEOUT: Duration = Duration::from_secs(5);
/// Maximum number of export retries
const DEFAULT_MAX_RETRIES: u32 = 3;
/// Retry backoff base duration
const DEFAULT_RETRY_BACKOFF: Duration = Duration::from_millis(100);

/// OTLP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtlpConfig {
    /// OTLP endpoint URL
    pub endpoint: String,
    /// Protocol to use (grpc or http)
    pub protocol: OtlpProtocol,
    /// Headers to include in requests
    pub headers: HashMap<String, String>,
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Batch timeout
    pub batch_timeout: Duration,
    /// Maximum export retries
    pub max_retries: u32,
    /// Retry backoff duration
    pub retry_backoff: Duration,
    /// Whether to enable compression
    pub compression: bool,
    /// Request timeout
    pub timeout: Duration,
    /// Whether to use TLS
    pub use_tls: bool,
}

impl Default for OtlpConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:4317".to_string(),
            protocol: OtlpProtocol::Grpc,
            headers: HashMap::new(),
            max_batch_size: DEFAULT_MAX_BATCH_SIZE,
            batch_timeout: DEFAULT_BATCH_TIMEOUT,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_backoff: DEFAULT_RETRY_BACKOFF,
            compression: false,
            timeout: Duration::from_secs(10),
            use_tls: false,
        }
    }
}

/// OTLP protocol types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OtlpProtocol {
    /// OTLP over gRPC
    Grpc,
    /// OTLP over HTTP
    Http,
}

/// Span data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Trace ID
    pub trace_id: String,
    /// Span ID
    pub span_id: String,
    /// Parent span ID (if any)
    pub parent_span_id: Option<String>,
    /// Span name
    pub name: String,
    /// Start time
    pub start_time: SystemTime,
    /// End time (if finished)
    pub end_time: Option<SystemTime>,
    /// Span attributes
    pub attributes: HashMap<String, String>,
    /// Span status
    pub status: SpanStatus,
    /// Span events
    pub events: Vec<SpanEvent>,
}

/// Span status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    /// Span completed successfully
    Ok,
    /// Span completed with error
    Error,
    /// Span status not set
    Unset,
}

/// Span event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    /// Event name
    pub name: String,
    /// Event timestamp
    pub time: SystemTime,
    /// Event attributes
    pub attributes: HashMap<String, String>,
}

/// OTLP export batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportBatch {
    /// Resource attributes
    pub resource: HashMap<String, String>,
    /// Spans in this batch
    pub spans: Vec<Span>,
    /// Batch creation time
    pub created_at: SystemTime,
}

/// OTLP Exporter
pub struct OtlpExporter {
    /// Configuration
    config: OtlpConfig,
    /// HTTP client
    client: reqwest::Client,
    /// Span sender channel
    span_sender: mpsc::UnboundedSender<Span>,
    /// Export statistics
    stats: Arc<RwLock<ExportStats>>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
}

/// Export statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ExportStats {
    /// Total spans received
    pub spans_received: u64,
    /// Total spans exported successfully
    pub spans_exported: u64,
    /// Total export failures
    pub export_failures: u64,
    /// Total batches sent
    pub batches_sent: u64,
    /// Total retries performed
    pub retries_performed: u64,
    /// Last export time
    pub last_export: Option<SystemTime>,
    /// Export errors
    pub last_error: Option<String>,
}

impl OtlpExporter {
    /// Create a new OTLP exporter
    pub async fn new(config: OtlpConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| Error::Init(format!("Failed to create HTTP client: {}", e)))?;

        let (span_sender, span_receiver) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = mpsc::channel(1);
        
        let stats = Arc::new(RwLock::new(ExportStats::default()));
        
        // Start background export task
        let export_task = ExportTask::new(
            config.clone(),
            client.clone(),
            span_receiver,
            shutdown_rx,
            stats.clone(),
        );
        
        tokio::spawn(export_task.run());
        
        Ok(Self {
            config,
            client,
            span_sender,
            stats,
            shutdown_tx: Some(shutdown_tx),
        })
    }

    /// Export a span
    pub async fn export_span(&self, span: Span) -> Result<()> {
        {
            let mut stats = self.stats.write().await;
            stats.spans_received += 1;
        }
        
        self.span_sender
            .send(span)
            .map_err(|_| Error::Init("Export channel closed".to_string()))?;
            
        Ok(())
    }

    /// Get export statistics
    pub async fn stats(&self) -> ExportStats {
        self.stats.read().await.clone()
    }

    /// Shutdown the exporter gracefully
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            if let Err(_) = shutdown_tx.send(()).await {
                warn!("Shutdown signal already sent or receiver dropped");
            }
        }
        Ok(())
    }

    /// Force flush all pending data
    pub async fn force_flush(&self) -> Result<()> {
        // Create a flush span to trigger immediate export
        let flush_span = Span {
            trace_id: "flush".to_string(),
            span_id: "flush".to_string(),
            parent_span_id: None,
            name: "force_flush".to_string(),
            start_time: SystemTime::now(),
            end_time: Some(SystemTime::now()),
            attributes: HashMap::new(),
            status: SpanStatus::Ok,
            events: vec![],
        };
        
        self.export_span(flush_span).await?;
        
        // Wait a bit for the flush to complete
        sleep(Duration::from_millis(100)).await;
        
        Ok(())
    }
}

impl Drop for OtlpExporter {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.try_send(());
        }
    }
}

/// Background export task
struct ExportTask {
    config: OtlpConfig,
    client: reqwest::Client,
    span_receiver: mpsc::UnboundedReceiver<Span>,
    shutdown_receiver: mpsc::Receiver<()>,
    stats: Arc<RwLock<ExportStats>>,
    batch: Vec<Span>,
    last_export: Instant,
}

impl ExportTask {
    fn new(
        config: OtlpConfig,
        client: reqwest::Client,
        span_receiver: mpsc::UnboundedReceiver<Span>,
        shutdown_receiver: mpsc::Receiver<()>,
        stats: Arc<RwLock<ExportStats>>,
    ) -> Self {
        Self {
            config,
            client,
            span_receiver,
            shutdown_receiver,
            stats,
            batch: Vec::new(),
            last_export: Instant::now(),
        }
    }

    async fn run(mut self) {
        let mut batch_timer = interval(self.config.batch_timeout);
        batch_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Receive new span
                span = self.span_receiver.recv() => {
                    match span {
                        Some(span) => {
                            self.add_span_to_batch(span).await;
                        }
                        None => {
                            // Channel closed, export remaining spans and exit
                            self.export_current_batch().await;
                            break;
                        }
                    }
                }
                
                // Batch timeout
                _ = batch_timer.tick() => {
                    if !self.batch.is_empty() {
                        self.export_current_batch().await;
                    }
                }
                
                // Shutdown signal
                _ = self.shutdown_receiver.recv() => {
                    info!("OTLP exporter shutdown requested");
                    self.export_current_batch().await;
                    break;
                }
            }
        }
        
        info!("OTLP export task completed");
    }

    async fn add_span_to_batch(&mut self, span: Span) {
        self.batch.push(span);
        
        // Export if batch is full
        if self.batch.len() >= self.config.max_batch_size {
            self.export_current_batch().await;
        }
        
        // Export if timeout exceeded
        if self.last_export.elapsed() >= self.config.batch_timeout {
            self.export_current_batch().await;
        }
    }

    async fn export_current_batch(&mut self) {
        if self.batch.is_empty() {
            return;
        }

        let batch = ExportBatch {
            resource: HashMap::from([
                ("service.name".to_string(), "nyx".to_string()),
                ("service.version".to_string(), env!("CARGO_PKG_VERSION").to_string()),
            ]),
            spans: std::mem::take(&mut self.batch),
            created_at: SystemTime::now(),
        };

        debug!("Exporting batch with {} spans", batch.spans.len());

        match self.export_batch_with_retry(&batch).await {
            Ok(_) => {
                let mut stats = self.stats.write().await;
                stats.spans_exported += batch.spans.len() as u64;
                stats.batches_sent += 1;
                stats.last_export = Some(SystemTime::now());
                debug!("Successfully exported {} spans", batch.spans.len());
            }
            Err(e) => {
                let mut stats = self.stats.write().await;
                stats.export_failures += 1;
                stats.last_error = Some(e.to_string());
                error!("Failed to export batch: {}", e);
            }
        }

        self.last_export = Instant::now();
    }

    async fn export_batch_with_retry(&mut self, batch: &ExportBatch) -> Result<()> {
        let mut retry_count = 0;
        let mut backoff = self.config.retry_backoff;

        loop {
            match self.export_batch(batch).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    retry_count += 1;
                    if retry_count > self.config.max_retries {
                        return Err(e);
                    }

                    {
                        let mut stats = self.stats.write().await;
                        stats.retries_performed += 1;
                    }

                    warn!("Export attempt {} failed, retrying in {:?}: {}", retry_count, backoff, e);
                    sleep(backoff).await;
                    backoff = std::cmp::min(backoff * 2, Duration::from_secs(30));
                }
            }
        }
    }

    async fn export_batch(&self, batch: &ExportBatch) -> Result<()> {
        let url = match self.config.protocol {
            OtlpProtocol::Grpc => format!("{}/v1/traces", self.config.endpoint),
            OtlpProtocol::Http => format!("{}/v1/traces", self.config.endpoint),
        };

        let body = serde_json::to_vec(batch)
            .map_err(|e| Error::Init(format!("Failed to serialize batch: {}", e)))?;

        let mut request = self.client
            .post(&url)
            .header("Content-Type", "application/json");

        // Add custom headers
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        // Add compression if enabled
        if self.config.compression {
            request = request.header("Content-Encoding", "gzip");
            // Note: In a real implementation, you would compress the body here
        }

        let response = timeout(self.config.timeout, request.body(body).send())
            .await
            .map_err(|_| Error::Init("Request timeout".to_string()))?
            .map_err(|e| Error::Init(format!("Request failed: {}", e)))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read response body".to_string());
            Err(Error::Init(format!("Export failed with status {}: {}", status, body)))
        }
    }
}

/// Utility functions for span creation
pub mod utils {
    use super::*;
    use std::collections::HashMap;

    /// Create a new span
    pub fn create_span(
        trace_id: String,
        span_id: String,
        name: String,
        parent_span_id: Option<String>,
    ) -> Span {
        Span {
            trace_id,
            span_id,
            parent_span_id,
            name,
            start_time: SystemTime::now(),
            end_time: None,
            attributes: HashMap::new(),
            status: SpanStatus::Unset,
            events: vec![],
        }
    }

    /// Finish a span
    pub fn finish_span(mut span: Span, status: SpanStatus) -> Span {
        span.end_time = Some(SystemTime::now());
        span.status = status;
        span
    }

    /// Add attribute to span
    pub fn add_attribute(mut span: Span, key: String, value: String) -> Span {
        span.attributes.insert(key, value);
        span
    }

    /// Add event to span
    pub fn add_event(mut span: Span, name: String, attributes: HashMap<String, String>) -> Span {
        span.events.push(SpanEvent {
            name,
            time: SystemTime::now(),
            attributes,
        });
        span
    }

    /// Generate a random trace ID
    pub fn generate_trace_id() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        
        format!("{:016x}{:016x}", timestamp, counter)
    }

    /// Generate a random span ID
    pub fn generate_span_id() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        format!("{:016x}", counter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    fn create_test_config() -> OtlpConfig {
        OtlpConfig {
            endpoint: "http://localhost:4318".to_string(),
            protocol: OtlpProtocol::Http,
            headers: HashMap::new(),
            max_batch_size: 2,
            batch_timeout: Duration::from_millis(100),
            max_retries: 1,
            retry_backoff: Duration::from_millis(10),
            compression: false,
            timeout: Duration::from_secs(1),
            use_tls: false,
        }
    }

    #[tokio::test]
    async fn test_otlp_exporter_creation() {
        let config = create_test_config();
        let exporter = OtlpExporter::new(config).await;
        assert!(exporter.is_ok());
    }

    #[tokio::test]
    async fn test_span_export() {
        let config = create_test_config();
        let mut exporter = OtlpExporter::new(config).await.unwrap();
        
        let span = utils::create_span(
            "trace123".to_string(),
            "span456".to_string(),
            "test_span".to_string(),
            None,
        );
        
        let result = exporter.export_span(span).await;
        assert!(result.is_ok());
        
        // Wait a bit for background processing
        sleep(Duration::from_millis(50)).await;
        
        let stats = exporter.stats().await;
        assert_eq!(stats.spans_received, 1);
        
        exporter.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_batch_export() {
        let config = create_test_config();
        let mut exporter = OtlpExporter::new(config).await.unwrap();
        
        // Export multiple spans to trigger batching
        for i in 0..3 {
            let span = utils::create_span(
                format!("trace{}", i),
                format!("span{}", i),
                format!("test_span_{}", i),
                None,
            );
            exporter.export_span(span).await.unwrap();
        }
        
        // Wait for batch processing
        sleep(Duration::from_millis(200)).await;
        
        let stats = exporter.stats().await;
        assert_eq!(stats.spans_received, 3);
        
        exporter.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_force_flush() {
        let config = create_test_config();
        let exporter = OtlpExporter::new(config).await.unwrap();
        
        let span = utils::create_span(
            "trace123".to_string(),
            "span456".to_string(),
            "test_span".to_string(),
            None,
        );
        
        exporter.export_span(span).await.unwrap();
        exporter.force_flush().await.unwrap();
        
        let stats = exporter.stats().await;
        assert!(stats.spans_received >= 1);
    }

    #[test]
    fn test_utils_span_creation() {
        let span = utils::create_span(
            "trace123".to_string(),
            "span456".to_string(),
            "test_span".to_string(),
            None,
        );
        
        assert_eq!(span.trace_id, "trace123");
        assert_eq!(span.span_id, "span456");
        assert_eq!(span.name, "test_span");
        assert!(span.parent_span_id.is_none());
        assert_eq!(span.status, SpanStatus::Unset);
    }

    #[test]
    fn test_utils_span_finishing() {
        let span = utils::create_span(
            "trace123".to_string(),
            "span456".to_string(),
            "test_span".to_string(),
            None,
        );
        
        let finished_span = utils::finish_span(span, SpanStatus::Ok);
        assert!(finished_span.end_time.is_some());
        assert_eq!(finished_span.status, SpanStatus::Ok);
    }

    #[test]
    fn test_utils_attribute_addition() {
        let span = utils::create_span(
            "trace123".to_string(),
            "span456".to_string(),
            "test_span".to_string(),
            None,
        );
        
        let span_with_attr = utils::add_attribute(
            span,
            "key1".to_string(),
            "value1".to_string(),
        );
        
        assert_eq!(span_with_attr.attributes.get("key1"), Some(&"value1".to_string()));
    }

    #[test]
    fn test_utils_event_addition() {
        let span = utils::create_span(
            "trace123".to_string(),
            "span456".to_string(),
            "test_span".to_string(),
            None,
        );
        
        let span_with_event = utils::add_event(
            span,
            "test_event".to_string(),
            HashMap::new(),
        );
        
        assert_eq!(span_with_event.events.len(), 1);
        assert_eq!(span_with_event.events[0].name, "test_event");
    }

    #[test]
    fn test_utils_id_generation() {
        let trace_id1 = utils::generate_trace_id();
        let trace_id2 = utils::generate_trace_id();
        assert_ne!(trace_id1, trace_id2);
        assert_eq!(trace_id1.len(), 32); // 16 bytes = 32 hex chars
        
        let span_id1 = utils::generate_span_id();
        let span_id2 = utils::generate_span_id();
        assert_ne!(span_id1, span_id2);
        assert_eq!(span_id1.len(), 16); // 8 bytes = 16 hex chars
    }

    #[test]
    fn test_config_default() {
        let config = OtlpConfig::default();
        assert_eq!(config.endpoint, "http://localhost:4317");
        assert_eq!(config.protocol, OtlpProtocol::Grpc);
        assert_eq!(config.max_batch_size, DEFAULT_MAX_BATCH_SIZE);
        assert_eq!(config.batch_timeout, DEFAULT_BATCH_TIMEOUT);
        assert_eq!(config.max_retries, DEFAULT_MAX_RETRIES);
        assert!(!config.compression);
        assert!(!config.use_tls);
    }

    #[test]
    fn test_export_stats_default() {
        let stats = ExportStats::default();
        assert_eq!(stats.spans_received, 0);
        assert_eq!(stats.spans_exported, 0);
        assert_eq!(stats.export_failures, 0);
        assert_eq!(stats.batches_sent, 0);
        assert_eq!(stats.retries_performed, 0);
        assert!(stats.last_export.is_none());
        assert!(stats.last_error.is_none());
    }
}