#![forbid(unsafe_code)]

//! Simplified multipath integration for testing
//! This is a basic integration layer to validate multipath functionality

use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::manager::{MultipathManager, MultipathPacket};
use super::simple_frame::{SimpleFrame, SimpleFrameType};
use crate::frame::FLAG_HAS_PATH_ID;

/// Statistics for the integration layer
#[derive(Debug, Clone)]
pub struct IntegrationStats {
    pub frames_processed: u64,
    pub frames_with_path_id: u64,
    pub frames_without_path_id: u64,
    pub multipath_packets_sent: u64,
    pub multipath_packets_received: u64,
    pub conversion_errors: u64,
}

/// Simplified multipath integration for testing
pub struct SimplifiedMultipathIntegration {
    /// Multipath manager
    manager: Arc<MultipathManager>,
    /// Integration statistics  
    stats: Arc<Mutex<IntegrationStats>>,
}

impl SimplifiedMultipathIntegration {
    /// Create new simplified integration
    pub fn new(manager: Arc<MultipathManager>) -> Self {
        let stats = Arc::new(Mutex::new(IntegrationStats {
            frames_processed: 0,
            frames_with_path_id: 0,
            frames_without_path_id: 0,
            multipath_packets_sent: 0,
            multipath_packets_received: 0,
            conversion_errors: 0,
        }));

        Self { manager, stats }
    }

    /// Send frame through multipath
    pub async fn send_frame(&self, frame: SimpleFrame) -> Result<SimpleFrame, String> {
        let mut stats = self.stats.lock().unwrap();
        stats.frames_processed += 1;

        // Check if frame has multipath metadata
        if frame.flags & FLAG_HAS_PATH_ID != 0 {
            stats.frames_with_path_id += 1;
        } else {
            stats.frames_without_path_id += 1;
        }

        drop(stats);

        // Send through multipath manager
        let packet = self.manager.send_packet(frame.data.clone()).await?;

        // Convert back to frame
        let output_frame = self.packet_to_frame(&packet)?;

        let mut stats = self.stats.lock().unwrap();
        stats.multipath_packets_sent += 1;

        Ok(output_frame)
    }

    /// Receive frame through multipath  
    pub async fn receive_frame(&self, frame: SimpleFrame) -> Result<Vec<SimpleFrame>, String> {
        // Extract multipath packet from frame
        let packet = self.frame_to_packet(&frame)?;

        // Process through multipath manager
        let ready_packets = self.manager.receive_packet(packet).await?;

        // Convert back to frames
        let mut frames = Vec::new();
        for packet in ready_packets {
            let frame = self.packet_to_frame(&packet)?;
            frames.push(frame);
        }

        let mut stats = self.stats.lock().unwrap();
        stats.multipath_packets_received += frames.len() as u64;

        Ok(frames)
    }

    /// Convert frame to multipath packet
    fn frame_to_packet(&self, frame: &SimpleFrame) -> Result<MultipathPacket, String> {
        // Extract path ID from extended headers
        let path_id = if let Some(path_id_str) = frame.extended_headers.get("path_id") {
            path_id_str.parse().map_err(|_| "Invalid path ID")?
        } else {
            return Err("No path ID in frame".to_string());
        };

        // Extract sequence from frame data (first 4 bytes)
        let sequence = if frame.data.len() >= 4 {
            let bytes = [frame.data[0], frame.data[1], frame.data[2], frame.data[3]];
            u32::from_be_bytes(bytes) as u64
        } else {
            return Err("Frame data too short for sequence".to_string());
        };

        // Extract hop count
        let hop_count = if let Some(hop_str) = frame.extended_headers.get("hop_count") {
            hop_str.parse().unwrap_or(5)
        } else {
            5
        };

        let data = if frame.data.len() > 4 {
            frame.data[4..].to_vec()
        } else {
            Vec::new()
        };

        Ok(MultipathPacket {
            path_id,
            sequence,
            data,
            sent_at: Instant::now(),
            hop_count,
        })
    }

    /// Convert multipath packet to frame
    fn packet_to_frame(&self, packet: &MultipathPacket) -> Result<SimpleFrame, String> {
        // Prepare frame data with sequence number prefix
        let mut frame_data = Vec::with_capacity(packet.data.len() + 4);
        frame_data.extend_from_slice(&(packet.sequence as u32).to_be_bytes());
        frame_data.extend_from_slice(&packet.data);

        // Create frame
        let mut frame = SimpleFrame::new(SimpleFrameType::Data, frame_data);

        // Add multipath metadata
        frame.set_flag(FLAG_HAS_PATH_ID);
        frame.add_extended_header("path_id", &packet.path_id.to_string())?;
        frame.add_extended_header("hop_count", &packet.hop_count.to_string())?;
        frame.add_extended_header("sent_at", &packet.sent_at.elapsed().as_millis().to_string())?;

        Ok(frame)
    }

    /// Get current statistics
    pub fn get_stats(&self) -> IntegrationStats {
        self.stats.lock().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simplified_integration() {
        use nyx_core::config::MultipathConfig;

        let config = MultipathConfig::default();
        let manager = Arc::new(MultipathManager::new_test(config));
        let integration = SimplifiedMultipathIntegration::new(manager.clone());

        // Add a test path
        manager.add_path(1).await.expect("Failed to add path");

        // Create test frame
        let frame = SimpleFrame::new(SimpleFrameType::Data, vec![1, 2, 3, 4, 5]);

        // Send frame
        let sent_frame = integration
            .send_frame(frame)
            .await
            .expect("Failed to send frame");

        // Frame should have PathID flag set
        assert_ne!(sent_frame.flags & FLAG_HAS_PATH_ID, 0);
        assert!(sent_frame.extended_headers.contains_key("path_id"));

        // Receive the frame back
        let received_frames = integration
            .receive_frame(sent_frame)
            .await
            .expect("Failed to receive frame");
        assert_eq!(received_frames.len(), 1);

        let stats = integration.get_stats();
        assert_eq!(stats.frames_processed, 1);
        assert_eq!(stats.multipath_packets_sent, 1);
        assert_eq!(stats.multipath_packets_received, 1);
    }
}
