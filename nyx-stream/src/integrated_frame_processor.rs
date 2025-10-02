//! Integrated Frame Processor for Nyx Stream Layer
//! Provides unified frame processing with zero-copy optimizations, reordering, and flow control
//! This module integrates frame parsing, validation, reordering, and congestion control

use crate::errors::{Error, Result};
use crate::frame::{Frame, FrameType};
use crate::frame_codec::FrameCodec;
use crate::telemetry_schema::{
    ConnectionId, NyxTelemetryInstrumentation, SpanStatus, TelemetryConfig,
};
use bytes::{Bytes, BytesMut};
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;
use tracing::{debug, error, warn};

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    /// Maximum frames to buffer for reordering
    pub max_reorder_buffer: usize,
    /// Timeout for frame processing operations
    pub processing_timeout: Duration,
    /// Maximum frame size to process
    pub max_frame_size: usize,
    /// Enable zero-copy optimizations
    pub zero_copy_enabled: bool,
    /// Buffer pool size for frame processing
    pub buffer_pool_size: usize,
    /// Reordering window size
    pub reordering_window: u64,
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            max_reorder_buffer: 1000,
            processing_timeout: Duration::from_millis(100),
            max_frame_size: 1280,
            zero_copy_enabled: true,
            buffer_pool_size: 64,
            reordering_window: 1000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProcessingMetrics {
    pub frames_processed: u64,
    pub frames_reordered: u64,
    pub frames_dropped: u64,
    pub processing_errors: u64,
    pub avg_processing_time: Duration,
    pub buffer_utilization: f64,
    pub last_reset: Instant,
}

impl Default for ProcessingMetrics {
    fn default() -> Self {
        Self {
            frames_processed: 0,
            frames_reordered: 0,
            frames_dropped: 0,
            processing_errors: 0,
            avg_processing_time: Duration::from_micros(0),
            buffer_utilization: 0.0,
            last_reset: Instant::now(),
        }
    }
}

/// Buffer pool for efficient memory management
struct BufferPool {
    buffers: Vec<BytesMut>,
    capacity: usize,
}

impl BufferPool {
    fn new(size: usize, buffer_capacity: usize) -> Self {
        let buffers = (0..size)
            .map(|_| BytesMut::with_capacity(buffer_capacity))
            .collect();

        Self {
            buffers,
            capacity: buffer_capacity,
        }
    }

    fn get_buffer(&mut self) -> BytesMut {
        self.buffers
            .pop()
            .unwrap_or_else(|| BytesMut::with_capacity(self.capacity))
    }

    fn return_buffer(&mut self, mut buffer: BytesMut) {
        buffer.clear();
        if buffer.capacity() == self.capacity && self.buffers.len() < 100 {
            self.buffers.push(buffer);
        }
    }
}

/// Reordering buffer for handling out-of-order frames
struct ReorderingBuffer {
    buffer: BTreeMap<u64, Frame>, // seq -> frame
    expected_seq: u64,
    window_size: u64,
    max_size: usize,
}

impl ReorderingBuffer {
    fn new(expected_seq: u64, window_size: u64, max_size: usize) -> Self {
        Self {
            buffer: BTreeMap::new(),
            expected_seq,
            window_size,
            max_size,
        }
    }

    fn insert(&mut self, frame: Frame) -> Vec<Frame> {
        let seq = frame.header.seq;
        let mut ready_frames = Vec::new();

        // Check if frame is within acceptable window
        if seq < self.expected_seq || seq >= self.expected_seq + self.window_size {
            debug!(
                "Frame seq {} outside window [{}, {})",
                seq,
                self.expected_seq,
                self.expected_seq + self.window_size
            );
            return ready_frames;
        }

        // Insert frame
        self.buffer.insert(seq, frame);

        // Enforce buffer size limit
        while self.buffer.len() > self.max_size {
            if let Some((oldest_seq, _)) = self.buffer.iter().next() {
                let oldest_seq = *oldest_seq;
                self.buffer.remove(&oldest_seq);
                debug!("Dropped frame seq {} due to buffer limit", oldest_seq);
            }
        }

        // Extract consecutive frames starting from expected_seq
        while let Some(frame) = self.buffer.remove(&self.expected_seq) {
            ready_frames.push(frame);
            self.expected_seq += 1;
        }

        ready_frames
    }

    fn get_buffer_info(&self) -> (usize, u64, u64) {
        (self.buffer.len(), self.expected_seq, self.window_size)
    }
}

/// Integrated Frame Processor with advanced processing capabilities
pub struct IntegratedFrameProcessor {
    config: ProcessorConfig,
    metrics: Arc<Mutex<ProcessingMetrics>>,
    buffer_pool: Arc<Mutex<BufferPool>>,
    reordering_buffers: Arc<RwLock<std::collections::HashMap<u32, ReorderingBuffer>>>, // stream_id -> buffer
    processing_times: Arc<Mutex<VecDeque<Duration>>>,
    /// Telemetry instrumentation for observability (Section 6.2)
    telemetry: Arc<NyxTelemetryInstrumentation>,
    /// Connection ID for telemetry span association
    connection_id: ConnectionId,
}

impl IntegratedFrameProcessor {
    pub fn new(config: ProcessorConfig) -> Self {
        let buffer_pool = BufferPool::new(config.buffer_pool_size, config.max_frame_size * 2);
        let telemetry_config = TelemetryConfig::default();
        let telemetry = Arc::new(NyxTelemetryInstrumentation::new(telemetry_config));
        let connection_id = ConnectionId::new(0); // Default CID, should be set via with_connection_id

        Self {
            config,
            metrics: Arc::new(Mutex::new(ProcessingMetrics::default())),
            buffer_pool: Arc::new(Mutex::new(buffer_pool)),
            reordering_buffers: Arc::new(RwLock::new(std::collections::HashMap::new())),
            processing_times: Arc::new(Mutex::new(VecDeque::new())),
            telemetry,
            connection_id,
        }
    }

    /// Set connection ID for telemetry span association
    pub fn with_connection_id(mut self, connection_id: u64) -> Self {
        self.connection_id = ConnectionId::new(connection_id);
        self
    }

    /// Process a raw byte buffer into frames with reordering
    pub async fn process_buffer(&self, data: Bytes) -> Result<Vec<Frame>> {
        // Telemetry: Create span for buffer processing (Section 6.2 - Frame receive instrumentation)
        let span_id = self
            .telemetry
            .get_context()
            .create_span("frame_buffer_processing", None)
            .await;

        if let Some(sid) = span_id {
            self.telemetry
                .get_context()
                .add_span_attribute(sid, "buffer.size", &data.len().to_string())
                .await;
            self.telemetry
                .get_context()
                .associate_connection(self.connection_id, sid)
                .await;
        }

        let start_time = Instant::now();
        let mut processed_frames = Vec::new();

        // Work on a single mutable buffer so we actually consume decoded bytes
        let mut buf = BytesMut::from(&data[..]);

        // Timeout wrapper for processing
        let result = timeout(self.config.processing_timeout, async {
            loop {
                match FrameCodec::decode(&mut buf) {
                    Ok(Some(frame)) => {
                        let reordered_frames = self.handle_frame_reordering(frame).await?;
                        processed_frames.extend(reordered_frames);
                        // Continue to decode remaining bytes
                        continue;
                    }
                    Ok(None) => {
                        // No complete frame available in remaining buffer
                        break;
                    }
                    Err(e) => {
                        error!("Frame decoding error: {}", e);
                        self.increment_error_counter().await;
                        return Err(e);
                    }
                }
            }
            Ok(processed_frames)
        })
        .await;

        let processing_time = start_time.elapsed();
        self.update_processing_metrics(processing_time, result.is_ok())
            .await;

        // Telemetry: End span with status (Section 6.2)
        if let Some(sid) = span_id {
            let status = if result.is_ok() {
                SpanStatus::Ok
            } else {
                SpanStatus::Error
            };
            self.telemetry.get_context().end_span(sid, status).await;
            if let Ok(Ok(ref frames)) = result {
                self.telemetry
                    .get_context()
                    .add_span_attribute(sid, "frames.processed", &frames.len().to_string())
                    .await;
            }
        }

        match result {
            Ok(Ok(frames)) => Ok(frames),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                warn!(
                    "Frame processing timeout after {:?}",
                    self.config.processing_timeout
                );
                Err(Error::ProcessingTimeout)
            }
        }
    }

    /// Process a single frame with validation and optimization
    pub async fn process_frame(&self, frame: Frame) -> Result<Vec<Frame>> {
        // Telemetry: Create span for single frame processing (Section 6.2)
        let span_id = self
            .telemetry
            .get_context()
            .create_span("frame_processing", None)
            .await;

        if let Some(sid) = span_id {
            self.telemetry
                .get_context()
                .add_span_attribute(sid, "frame.type", &format!("{:?}", frame.header.ty))
                .await;
            self.telemetry
                .get_context()
                .add_span_attribute(sid, "frame.stream_id", &frame.header.stream_id.to_string())
                .await;
            self.telemetry
                .get_context()
                .add_span_attribute(sid, "frame.seq", &frame.header.seq.to_string())
                .await;
            self.telemetry
                .get_context()
                .associate_connection(self.connection_id, sid)
                .await;
        }

        let start_time = Instant::now();

        // Validate frame; count errors in metrics if invalid
        if let Err(e) = self.validate_frame(&frame) {
            self.increment_error_counter().await;
            if let Some(sid) = span_id {
                self.telemetry
                    .get_context()
                    .end_span(sid, SpanStatus::Error)
                    .await;
            }
            return Err(e);
        }

        // Handle reordering
        let reordered_frames = self.handle_frame_reordering(frame).await?;

        let processing_time = start_time.elapsed();
        self.update_processing_metrics(processing_time, true).await;

        // Telemetry: End span with success status (Section 6.2)
        if let Some(sid) = span_id {
            self.telemetry
                .get_context()
                .add_span_attribute(sid, "frames.reordered", &reordered_frames.len().to_string())
                .await;
            self.telemetry.get_context().end_span(sid, SpanStatus::Ok).await;
        }

        Ok(reordered_frames)
    }

    /// Encode frames to bytes with zero-copy optimization
    pub async fn encode_frames(&self, frames: &[Frame]) -> Result<Bytes> {
        // Telemetry: Create span for frame encoding/sending (Section 6.2 - Frame send instrumentation)
        let span_id = self
            .telemetry
            .get_context()
            .create_span("frame_encoding", None)
            .await;

        if let Some(sid) = span_id {
            self.telemetry
                .get_context()
                .add_span_attribute(sid, "frames.count", &frames.len().to_string())
                .await;
            self.telemetry
                .get_context()
                .associate_connection(self.connection_id, sid)
                .await;
        }

        let mut buffer_pool = self.buffer_pool.lock().await;
        let mut buffer = buffer_pool.get_buffer();

        for frame in frames {
            FrameCodec::encode(frame, &mut buffer)?;
        }

        let result = buffer.freeze();
        buffer_pool.return_buffer(BytesMut::new()); // Return empty buffer to pool

        // Telemetry: End span with encoded size (Section 6.2)
        if let Some(sid) = span_id {
            self.telemetry
                .get_context()
                .add_span_attribute(sid, "encoded.bytes", &result.len().to_string())
                .await;
            self.telemetry.get_context().end_span(sid, SpanStatus::Ok).await;
        }

        Ok(result)
    }

    /// Handle frame reordering logic
    async fn handle_frame_reordering(&self, frame: Frame) -> Result<Vec<Frame>> {
        let stream_id = frame.header.stream_id;
        let mut reordering_buffers = self.reordering_buffers.write().await;

        // Get or create reordering buffer for this stream
        let reordering_buffer = reordering_buffers.entry(stream_id).or_insert_with(|| {
            ReorderingBuffer::new(
                1, // Start from sequence 1
                self.config.reordering_window,
                self.config.max_reorder_buffer,
            )
        });

        let ready_frames = reordering_buffer.insert(frame);

        // Update metrics
        if !ready_frames.is_empty() {
            let mut metrics = self.metrics.lock().await;
            metrics.frames_processed += ready_frames.len() as u64;

            // Check if reordering occurred
            if ready_frames.len() > 1
                || (ready_frames.len() == 1
                    && ready_frames[0].header.seq != reordering_buffer.expected_seq - 1)
            {
                metrics.frames_reordered += 1;
            }
        }

        Ok(ready_frames)
    }

    /// Validate frame structure and content
    fn validate_frame(&self, frame: &Frame) -> Result<()> {
        // Check frame size
        if frame.payload.len() > self.config.max_frame_size {
            return Err(Error::InvalidFrame(format!(
                "Frame too large: {} bytes",
                frame.payload.len()
            )));
        }

        // Validate frame type specific constraints
        match frame.header.ty {
            FrameType::Ack => {
                if !frame.payload.is_empty() {
                    return Err(Error::InvalidFrame(
                        "ACK frame should have empty payload".to_string(),
                    ));
                }
            }
            FrameType::Close => {
                if !frame.payload.is_empty() {
                    return Err(Error::InvalidFrame(
                        "CLOSE frame should have empty payload".to_string(),
                    ));
                }
            }
            FrameType::Data => {
                // Data frames can have any payload size within limits
            }
            FrameType::Crypto => {
                // CRYPTO frames contain handshake payloads (ClientHello, ServerHello, ClientFinished)
                // Payload validation handled by handshake layer
            }
            FrameType::Custom(_) => {
                // Custom frames handled by plugin framework
                // Payload validation delegated to plugin implementation
            }
        }

        Ok(())
    }

    /// Update processing metrics
    async fn update_processing_metrics(&self, processing_time: Duration, success: bool) {
        let mut metrics = self.metrics.lock().await;
        let mut times = self.processing_times.lock().await;

        if success {
            times.push_back(processing_time);

            // Keep only recent times for average calculation
            while times.len() > 1000 {
                times.pop_front();
            }

            // Update average processing time
            if !times.is_empty() {
                let total: Duration = times.iter().sum();
                metrics.avg_processing_time = total / times.len() as u32;
            }
        } else {
            metrics.processing_errors += 1;
        }

        // Update buffer utilization
        let reordering_buffers = self.reordering_buffers.read().await;
        let total_buffered: usize = reordering_buffers
            .values()
            .map(|buf| buf.get_buffer_info().0)
            .sum();

        metrics.buffer_utilization =
            (total_buffered as f64) / (self.config.max_reorder_buffer as f64);
    }

    /// Increment error counter
    async fn increment_error_counter(&self) {
        let mut metrics = self.metrics.lock().await;
        metrics.processing_errors += 1;
    }

    /// Get current processing metrics
    pub async fn get_metrics(&self) -> ProcessingMetrics {
        self.metrics.lock().await.clone()
    }

    /// Reset metrics
    pub async fn reset_metrics(&self) {
        let mut metrics = self.metrics.lock().await;
        *metrics = ProcessingMetrics::default();

        let mut times = self.processing_times.lock().await;
        times.clear();
    }

    /// Get reordering buffer status for all streams
    pub async fn get_buffer_status(&self) -> std::collections::HashMap<u32, (usize, u64, u64)> {
        let buffers = self.reordering_buffers.read().await;
        buffers
            .iter()
            .map(|(&stream_id, buffer)| (stream_id, buffer.get_buffer_info()))
            .collect()
    }

    /// Force flush reordering buffers (useful for stream closure)
    pub async fn flush_stream_buffer(&self, stream_id: u32) -> Result<Vec<Frame>> {
        let mut reordering_buffers = self.reordering_buffers.write().await;

        if let Some(buffer) = reordering_buffers.remove(&stream_id) {
            // Extract all buffered frames
            let mut flushed_frames = Vec::new();
            for (_, frame) in buffer.buffer.into_iter() {
                flushed_frames.push(frame);
            }

            // Sort by sequence number for consistency
            flushed_frames.sort_by_key(|f| f.header.seq);

            let mut metrics = self.metrics.lock().await;
            metrics.frames_dropped += flushed_frames.len() as u64;

            Ok(flushed_frames)
        } else {
            Ok(Vec::new())
        }
    }

    /// Get processing statistics
    pub async fn get_processing_stats(&self) -> Result<String> {
        let metrics = self.get_metrics().await;
        let buffer_status = self.get_buffer_status().await;

        let active_streams = buffer_status
            .values()
            .filter(|(count, _, _)| *count > 0)
            .count();
        let total_buffered: usize = buffer_status.values().map(|(count, _, _)| count).sum();

        Ok(format!(
            "Processing Stats:\n\
             - Frames processed: {}\n\
             - Frames reordered: {}\n\
             - Frames dropped: {}\n\
             - Processing errors: {}\n\
             - Avg processing time: {:?}\n\
             - Buffer utilization: {:.2}%\n\
             - Active streams: {}\n\
             - Total buffered frames: {}",
            metrics.frames_processed,
            metrics.frames_reordered,
            metrics.frames_dropped,
            metrics.processing_errors,
            metrics.avg_processing_time,
            metrics.buffer_utilization * 100.0,
            active_streams,
            total_buffered
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::FrameBuilder;
    use crate::frame::{Frame, FrameHeader, FrameType};

    #[tokio::test]
    async fn test_processor_creation() {
        let config = ProcessorConfig::default();
        let processor = IntegratedFrameProcessor::new(config);

        let metrics = processor.get_metrics().await;
        assert_eq!(metrics.frames_processed, 0);
    }

    #[tokio::test]
    async fn test_frame_processing() -> Result<()> {
        let config = ProcessorConfig::default();
        let processor = IntegratedFrameProcessor::new(config);
        let builder = FrameBuilder::new();

        let frame = builder.build_data_frame(1, 1, b"test data");
        let processed = processor.process_frame(frame.clone()).await?;

        assert_eq!(processed.len(), 1);
        assert_eq!(processed[0].header.seq, frame.header.seq);

        Ok(())
    }

    #[tokio::test]
    async fn test_frame_reordering() -> Result<()> {
        let config = ProcessorConfig::default();
        let processor = IntegratedFrameProcessor::new(config);
        let builder = FrameBuilder::new();

        // Send frames out of order: 2, 1, 3
        let frame2 = builder.build_data_frame(1, 2, b"frame 2");
        let frame1 = builder.build_data_frame(1, 1, b"frame 1");
        let frame3 = builder.build_data_frame(1, 3, b"frame 3");

        // Process frame 2 first (should be buffered)
        let result2 = processor.process_frame(frame2).await?;
        assert!(result2.is_empty()); // Should be buffered

        // Process frame 1 (should release both 1 and 2)
        let result1 = processor.process_frame(frame1).await?;
        assert_eq!(result1.len(), 2);
        assert_eq!(result1[0].header.seq, 1);
        assert_eq!(result1[1].header.seq, 2);

        // Process frame 3 (should be released immediately)
        let result3 = processor.process_frame(frame3).await?;
        assert_eq!(result3.len(), 1);
        assert_eq!(result3[0].header.seq, 3);

        let metrics = processor.get_metrics().await;
        assert_eq!(metrics.frames_processed, 3);
        assert!(metrics.frames_reordered > 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_frame_validation() -> Result<()> {
        let config = ProcessorConfig {
            max_frame_size: 100,
            ..ProcessorConfig::default()
        };
        let processor = IntegratedFrameProcessor::new(config);

        // Test oversized frame
        let oversized_payload = vec![0u8; 200];
        let frame = Frame {
            header: FrameHeader {
                stream_id: 1,
                seq: 1,
                ty: FrameType::Data,
            },
            payload: oversized_payload,
        };

        let result = processor.process_frame(frame).await;
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_buffer_flushing() -> Result<()> {
        let config = ProcessorConfig::default();
        let processor = IntegratedFrameProcessor::new(config);
        let builder = FrameBuilder::new();

        // Add some frames to buffer (out of order)
        let frame3 = builder.build_data_frame(1, 3, b"frame 3");
        let frame2 = builder.build_data_frame(1, 2, b"frame 2");

        processor.process_frame(frame3).await?;
        processor.process_frame(frame2).await?;

        // Flush the buffer
        let flushed = processor.flush_stream_buffer(1).await?;
        assert_eq!(flushed.len(), 2);
        assert_eq!(flushed[0].header.seq, 2); // Should be sorted
        assert_eq!(flushed[1].header.seq, 3);

        Ok(())
    }

    #[tokio::test]
    async fn test_metrics_tracking() -> Result<()> {
        let config = ProcessorConfig::default();
        let processor = IntegratedFrameProcessor::new(config);
        let builder = FrameBuilder::new();

        // Process several frames
        for i in 1..=5 {
            let payload_str = format!("frame {i}");
            let frame = builder.build_data_frame(1, i, payload_str.as_bytes());
            processor.process_frame(frame).await?;
        }

        let metrics = processor.get_metrics().await;
        assert_eq!(metrics.frames_processed, 5);
        assert!(metrics.avg_processing_time > Duration::from_nanos(0));

        Ok(())
    }
}
