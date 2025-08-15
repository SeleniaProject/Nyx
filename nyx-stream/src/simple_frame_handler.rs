use std::time::Duration;

/// Simple frame handler for performance testing
#[derive(Debug)]
pub struct FrameHandler {
    pub max_frame_size: usize,
    pub timeout: Duration,
    // Per-stream statistics for basic observability
    streams: std::collections::HashMap<u64, StreamStats>,
}

#[derive(Debug, Clone, Copy)]
struct StreamStats {
    last_seen: std::time::Instant,
    bytes_processed: u64,
    frames_processed: usize,
}

impl FrameHandler {
    pub fn new(max_frame_size: usize, timeout: Duration) -> Self {
        Self {
            max_frame_size,
            timeout,
            streams: std::collections::HashMap::new(),
        }
    }

    /// Process a frame for performance testing (async compatible)
    pub async fn process_frame_async(
        &mut self,
        stream_id: u64,
        data: Vec<u8>,
    ) -> crate::errors::StreamResult<Option<Vec<u8>>> {
        if data.len() > self.max_frame_size {
            return Err(crate::errors::StreamError::InvalidFrame(format!(
                "Frame size {} exceeds maximum {}",
                data.len(),
                self.max_frame_size
            )));
        }

        // Track simple per-stream statistics
        let entry = self.streams.entry(stream_id).or_insert(StreamStats {
            last_seen: std::time::Instant::now(),
            bytes_processed: 0,
            frames_processed: 0,
        });
        entry.last_seen = std::time::Instant::now();
        entry.bytes_processed = entry.bytes_processed.saturating_add(data.len() as u64);
        entry.frames_processed = entry.frames_processed.saturating_add(1);

        // Simple processing for performance testing - just return the data (echo)
        Ok(Some(data))
    }

    /// Get number of active streams (always 0 for this simple implementation)
    pub fn active_streams(&self) -> usize {
        self.streams.len()
    }

    /// Clean up expired streams based on `timeout`
    pub fn cleanup_expired_streams(&mut self) {
        let now = std::time::Instant::now();
        let timeout = self.timeout;
        self.streams
            .retain(|_, s| now.duration_since(s.last_seen) <= timeout);
    }

    /// Close a stream and drop its statistics
    pub fn close_stream(&mut self, stream_id: u64) {
        let _ = self.streams.remove(&stream_id);
    }

    /// Get stream statistics (bytes_processed, frames_processed, reserved)
    pub fn get_stream_stats(&self, stream_id: u64) -> Option<(u64, usize, usize)> {
        self.streams
            .get(&stream_id)
            .map(|s| (s.bytes_processed, s.frames_processed, 0))
    }
}

impl Default for FrameHandler {
    fn default() -> Self {
        Self::new(16384, Duration::from_secs(60))
    }
}
