use crate::errors::Result;
use crate::frame::FrameBuilder;
use crate::integrated_frame_processor::{IntegratedFrameProcessor, ProcessorConfig};
use crate::{Frame, FrameCodec, FrameHeader, FrameType};
use bytes::BytesMut;
use std::time::{Duration, Instant};

/// Comprehensive tests for integrated frame processor functionality
/// Tests reordering, buffer management, performance, and error handling

#[tokio::test]
async fn test_processor_initialization() {
    let config = ProcessorConfig::default();
    let processor = IntegratedFrameProcessor::new(config.clone());

    let metrics = processor.get_metrics().await;
    assert_eq!(metrics.frames_processed, 0);
    assert_eq!(metrics.frames_reordered, 0);
    assert_eq!(metrics.processing_errors, 0);

    // Test custom configuration
    let custom_config = ProcessorConfig {
        max_reorder_buffer: 500,
        processing_timeout: Duration::from_millis(200),
        max_frame_size: 2000,
        zero_copy_enabled: true,
        buffer_pool_size: 32,
        reordering_window: 500,
    };

    let custom_processor = IntegratedFrameProcessor::new(custom_config);
    let custom_metrics = custom_processor.get_metrics().await;
    assert_eq!(custom_metrics.frames_processed, 0);
}

#[tokio::test]
async fn test_single_frame_processing() -> Result<()> {
    let config = ProcessorConfig::default();
    let processor = IntegratedFrameProcessor::new(config);
    let builder = FrameBuilder::new();

    // Test data frame
    let data_frame = builder.build_data_frame(1, 1, b"test data payload");
    let processed = processor.process_frame(data_frame.clone()).await?;

    assert_eq!(processed.len(), 1);
    assert_eq!(processed[0].header.stream_id, data_frame.header.stream_id);
    assert_eq!(processed[0].header.seq, data_frame.header.seq);
    assert_eq!(processed[0].header.ty, FrameType::Data);
    assert_eq!(processed[0].payload, data_frame.payload);

    // Test ACK frame
    let ack_frame = Frame {
        header: FrameHeader {
            stream_id: 1,
            seq: 2,
            ty: FrameType::Ack,
        },
        payload: vec![],
    };

    let ack_processed = processor.process_frame(ack_frame.clone()).await?;
    assert_eq!(ack_processed.len(), 1);
    assert_eq!(ack_processed[0].header.ty, FrameType::Ack);
    assert!(ack_processed[0].payload.is_empty());

    // Check metrics
    let metrics = processor.get_metrics().await;
    assert_eq!(metrics.frames_processed, 2);

    Ok(())
}

#[tokio::test]
async fn test_frame_reordering_basic() -> Result<()> {
    let config = ProcessorConfig::default();
    let processor = IntegratedFrameProcessor::new(config);
    let builder = FrameBuilder::new();

    // Send frames in order: 2, 1, 3 (out of order)
    let frame2 = builder.build_data_frame(1, 2, b"frame 2");
    let frame1 = builder.build_data_frame(1, 1, b"frame 1");
    let frame3 = builder.build_data_frame(1, 3, b"frame 3");

    // Process frame 2 first - should be buffered
    let result2 = processor.process_frame(frame2).await?;
    assert!(result2.is_empty(), "Frame 2 should be buffered");

    // Process frame 1 - should release both 1 and 2 in order
    let result1 = processor.process_frame(frame1).await?;
    assert_eq!(result1.len(), 2, "Should release frames 1 and 2");
    assert_eq!(result1[0].header.seq, 1, "First frame should be seq 1");
    assert_eq!(result1[1].header.seq, 2, "Second frame should be seq 2");

    // Process frame 3 - should be released immediately
    let result3 = processor.process_frame(frame3).await?;
    assert_eq!(result3.len(), 1, "Should release frame 3");
    assert_eq!(result3[0].header.seq, 3, "Should be seq 3");

    // Verify metrics
    let metrics = processor.get_metrics().await;
    assert_eq!(metrics.frames_processed, 3);
    assert!(
        metrics.frames_reordered > 0,
        "Should have detected reordering"
    );

    Ok(())
}

#[tokio::test]
async fn test_complex_reordering_scenario() -> Result<()> {
    let config = ProcessorConfig::default();
    let processor = IntegratedFrameProcessor::new(config);
    let builder = FrameBuilder::new();

    // Create frames in complex out-of-order sequence: 5, 2, 7, 1, 4, 3, 6
    let sequences = [5, 2, 7, 1, 4, 3, 6];
    let mut all_processed = Vec::new();

    for &seq in &sequences {
        let payload_str = format!("frame {seq}");
        let frame = builder.build_data_frame(1, seq, payload_str.as_bytes());
        let processed = processor.process_frame(frame).await?;
        all_processed.extend(processed);
    }

    // Verify all frames were eventually processed in order
    assert!(
        !all_processed.is_empty(),
        "Should have processed some frames"
    );

    // Check that processed frames are in sequence order
    for window in all_processed.windows(2) {
        assert!(
            window[0].header.seq < window[1].header.seq,
            "Frames should be in sequence order: {} < {}",
            window[0].header.seq,
            window[1].header.seq
        );
    }

    let metrics = processor.get_metrics().await;
    assert!(
        metrics.frames_reordered > 0,
        "Should have detected reordering"
    );

    Ok(())
}

#[tokio::test]
async fn test_multiple_streams_reordering() -> Result<()> {
    let config = ProcessorConfig::default();
    let processor = IntegratedFrameProcessor::new(config);
    let builder = FrameBuilder::new();

    // Test reordering across multiple streams
    let stream1_frame2 = builder.build_data_frame(1, 2, b"stream1 frame2");
    let stream2_frame2 = builder.build_data_frame(2, 2, b"stream2 frame2");
    let stream1_frame1 = builder.build_data_frame(1, 1, b"stream1 frame1");
    let stream2_frame1 = builder.build_data_frame(2, 1, b"stream2 frame1");

    // Process out-of-order frames for both streams
    let _result1 = processor.process_frame(stream1_frame2).await?;
    let _result2 = processor.process_frame(stream2_frame2).await?;

    // Both frame 2s should be buffered
    let buffer_status = processor.get_buffer_status().await;
    assert_eq!(buffer_status.len(), 2, "Should have buffers for 2 streams");

    // Process frame 1 for stream 1
    let stream1_results = processor.process_frame(stream1_frame1).await?;
    assert_eq!(
        stream1_results.len(),
        2,
        "Should release both stream 1 frames"
    );

    // Process frame 1 for stream 2
    let stream2_results = processor.process_frame(stream2_frame1).await?;
    assert_eq!(
        stream2_results.len(),
        2,
        "Should release both stream 2 frames"
    );

    Ok(())
}

#[tokio::test]
async fn test_buffer_processing() -> Result<()> {
    let config = ProcessorConfig::default();
    let processor = IntegratedFrameProcessor::new(config);
    let builder = FrameBuilder::new();

    // Create multiple frames and encode them
    let frames = vec![
        builder.build_data_frame(1, 1, b"frame 1"),
        builder.build_data_frame(1, 2, b"frame 2"),
        builder.build_data_frame(1, 3, b"frame 3"),
    ];

    let mut buffer = BytesMut::new();
    for frame in &frames {
        FrameCodec::encode(frame, &mut buffer)?;
    }

    let data = buffer.freeze();
    let processed_frames = processor.process_buffer(data).await?;

    assert_eq!(processed_frames.len(), 3, "Should process all 3 frames");
    for (i, frame) in processed_frames.iter().enumerate() {
        assert_eq!(
            frame.header.seq,
            (i + 1) as u64,
            "Frames should be in order"
        );
    }

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
    let oversized_frame = Frame {
        header: FrameHeader {
            stream_id: 1,
            seq: 1,
            ty: FrameType::Data,
        },
        payload: vec![0u8; 200], // Exceeds max_frame_size
    };

    let result = processor.process_frame(oversized_frame).await;
    assert!(result.is_err(), "Should reject oversized frame");

    // Test invalid ACK frame with payload
    let invalid_ack = Frame {
        header: FrameHeader {
            stream_id: 1,
            seq: 2,
            ty: FrameType::Ack,
        },
        payload: b"should be empty".to_vec(),
    };

    let result = processor.process_frame(invalid_ack).await;
    assert!(result.is_err(), "Should reject ACK frame with payload");

    // Test invalid CLOSE frame with payload
    let invalid_close = Frame {
        header: FrameHeader {
            stream_id: 1,
            seq: 3,
            ty: FrameType::Close,
        },
        payload: b"should be empty".to_vec(),
    };

    let result = processor.process_frame(invalid_close).await;
    assert!(result.is_err(), "Should reject CLOSE frame with payload");

    Ok(())
}

#[tokio::test]
async fn test_processing_timeout() -> Result<()> {
    let config = ProcessorConfig {
        processing_timeout: Duration::from_millis(1), // Very short timeout
        ..ProcessorConfig::default()
    };
    let processor = IntegratedFrameProcessor::new(config);

    // Create a large buffer that might take time to process
    let mut large_buffer = BytesMut::new();
    for _ in 0..1000 {
        large_buffer.extend_from_slice(&[0u8; 100]);
    }

    let data = large_buffer.freeze();

    // This should timeout due to the very short processing timeout
    // Note: This test might be flaky depending on system performance
    let start = Instant::now();
    let _result = processor.process_buffer(data).await;
    let elapsed = start.elapsed();

    // Either it completes very quickly or times out
    assert!(
        elapsed < Duration::from_millis(100),
        "Should either complete quickly or timeout"
    );

    Ok(())
}

#[tokio::test]
async fn test_metrics_tracking() -> Result<()> {
    let config = ProcessorConfig::default();
    let processor = IntegratedFrameProcessor::new(config);
    let builder = FrameBuilder::new();

    // Process multiple frames to generate metrics
    for i in 1..=10 {
        let payload_str = format!("frame {i}");
        let frame = builder.build_data_frame(1, i, payload_str.as_bytes());
        processor.process_frame(frame).await?;
    }

    let metrics = processor.get_metrics().await;
    assert_eq!(metrics.frames_processed, 10);
    assert!(metrics.avg_processing_time > Duration::from_nanos(0));
    assert_eq!(metrics.processing_errors, 0);

    // Test error counting
    let invalid_frame = Frame {
        header: FrameHeader {
            stream_id: 1,
            seq: 11,
            ty: FrameType::Data,
        },
        payload: vec![0u8; 2000], // Too large
    };

    let _result = processor.process_frame(invalid_frame).await;

    let updated_metrics = processor.get_metrics().await;
    assert!(updated_metrics.processing_errors > 0);

    Ok(())
}

#[tokio::test]
async fn test_buffer_status_monitoring() -> Result<()> {
    let config = ProcessorConfig::default();
    let processor = IntegratedFrameProcessor::new(config);
    let builder = FrameBuilder::new();

    // Create frames that will be buffered (out of order)
    let frame3 = builder.build_data_frame(1, 3, b"frame 3");
    let frame5 = builder.build_data_frame(2, 5, b"frame 5");

    processor.process_frame(frame3).await?;
    processor.process_frame(frame5).await?;

    let buffer_status = processor.get_buffer_status().await;
    assert_eq!(buffer_status.len(), 2, "Should have 2 stream buffers");

    // Check stream 1 buffer
    if let Some(&(count, expected_seq, _window)) = buffer_status.get(&1) {
        assert_eq!(count, 1, "Stream 1 should have 1 buffered frame");
        assert_eq!(expected_seq, 1, "Stream 1 expecting seq 1");
    } else {
        panic!("Stream 1 buffer not found");
    }

    // Check stream 2 buffer
    if let Some(&(count, expected_seq, _window)) = buffer_status.get(&2) {
        assert_eq!(count, 1, "Stream 2 should have 1 buffered frame");
        assert_eq!(expected_seq, 1, "Stream 2 expecting seq 1");
    } else {
        panic!("Stream 2 buffer not found");
    }

    Ok(())
}

#[tokio::test]
async fn test_buffer_flushing() -> Result<()> {
    let config = ProcessorConfig::default();
    let processor = IntegratedFrameProcessor::new(config);
    let builder = FrameBuilder::new();

    // Add frames to buffers for multiple streams
    let frames = vec![
        builder.build_data_frame(1, 3, b"stream1 frame3"),
        builder.build_data_frame(1, 5, b"stream1 frame5"),
        builder.build_data_frame(2, 2, b"stream2 frame2"),
        builder.build_data_frame(2, 4, b"stream2 frame4"),
    ];

    for frame in frames {
        processor.process_frame(frame).await?;
    }

    // Flush stream 1
    let flushed1 = processor.flush_stream_buffer(1).await?;
    assert_eq!(flushed1.len(), 2, "Should flush 2 frames from stream 1");
    assert_eq!(
        flushed1[0].header.seq, 3,
        "First flushed frame should be seq 3"
    );
    assert_eq!(
        flushed1[1].header.seq, 5,
        "Second flushed frame should be seq 5"
    );

    // Flush stream 2
    let flushed2 = processor.flush_stream_buffer(2).await?;
    assert_eq!(flushed2.len(), 2, "Should flush 2 frames from stream 2");
    assert_eq!(
        flushed2[0].header.seq, 2,
        "First flushed frame should be seq 2"
    );
    assert_eq!(
        flushed2[1].header.seq, 4,
        "Second flushed frame should be seq 4"
    );

    // Verify buffers are empty
    let buffer_status = processor.get_buffer_status().await;
    assert!(
        buffer_status.is_empty(),
        "All buffers should be empty after flushing"
    );

    Ok(())
}

#[tokio::test]
async fn test_frame_encoding() -> Result<()> {
    let config = ProcessorConfig::default();
    let processor = IntegratedFrameProcessor::new(config);
    let builder = FrameBuilder::new();

    let frames = vec![
        builder.build_data_frame(1, 1, b"frame 1"),
        builder.build_data_frame(1, 2, b"frame 2"),
        builder.build_data_frame(1, 3, b"frame 3"),
    ];

    let encoded_bytes = processor.encode_frames(&frames).await?;
    assert!(
        !encoded_bytes.is_empty(),
        "Encoded bytes should not be empty"
    );

    // Verify we can decode the frames back
    let mut bytes = BytesMut::from(&encoded_bytes[..]);
    let mut decoded_frames = Vec::new();

    while !bytes.is_empty() {
        if let Ok(Some(frame)) = FrameCodec::decode(&mut bytes) {
            decoded_frames.push(frame);
        } else {
            break;
        }
    }

    assert_eq!(decoded_frames.len(), 3, "Should decode all 3 frames");
    for (i, frame) in decoded_frames.iter().enumerate() {
        assert_eq!(
            frame.header.seq,
            (i + 1) as u64,
            "Frame sequence should match"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_metrics_reset() -> Result<()> {
    let config = ProcessorConfig::default();
    let processor = IntegratedFrameProcessor::new(config);
    let builder = FrameBuilder::new();

    // Process some frames
    for i in 1..=5 {
        let payload_str = format!("frame {i}");
        let frame = builder.build_data_frame(1, i, payload_str.as_bytes());
        processor.process_frame(frame).await?;
    }

    let metrics_before = processor.get_metrics().await;
    assert_eq!(metrics_before.frames_processed, 5);

    // Reset metrics
    processor.reset_metrics().await;

    let metrics_after = processor.get_metrics().await;
    assert_eq!(metrics_after.frames_processed, 0);
    assert_eq!(metrics_after.frames_reordered, 0);
    assert_eq!(metrics_after.processing_errors, 0);

    Ok(())
}

#[tokio::test]
async fn test_processing_stats_generation() -> Result<()> {
    let config = ProcessorConfig::default();
    let processor = IntegratedFrameProcessor::new(config);
    let builder = FrameBuilder::new();

    // Add some processing activity
    for i in 1..=3 {
        let payload_str = format!("frame {i}");
        let frame = builder.build_data_frame(1, i, payload_str.as_bytes());
        processor.process_frame(frame).await?;
    }

    // Add a buffered frame
    let buffered_frame = builder.build_data_frame(2, 5, b"buffered frame");
    processor.process_frame(buffered_frame).await?;

    let stats = processor.get_processing_stats().await?;
    assert!(stats.contains("Frames processed: 3"));
    assert!(stats.contains("Active streams: 1"));
    assert!(stats.contains("Total buffered frames: 1"));

    Ok(())
}

#[tokio::test]
async fn test_reordering_window_limits() -> Result<()> {
    let config = ProcessorConfig {
        reordering_window: 5, // Small window for testing
        ..ProcessorConfig::default()
    };
    let processor = IntegratedFrameProcessor::new(config);
    let builder = FrameBuilder::new();

    // Try to process a frame far outside the window
    let far_frame = builder.build_data_frame(1, 10, b"far frame"); // Outside window [1, 6)
    let result = processor.process_frame(far_frame).await?;

    // Frame should be dropped (not processed)
    assert!(result.is_empty(), "Frame outside window should be dropped");

    // Process frame within window
    let near_frame = builder.build_data_frame(1, 3, b"near frame");
    let result = processor.process_frame(near_frame).await?;

    // Frame should be buffered
    assert!(result.is_empty(), "Frame within window should be buffered");

    let buffer_status = processor.get_buffer_status().await;
    assert_eq!(
        buffer_status.get(&1).unwrap().0,
        1,
        "Should have 1 buffered frame"
    );

    Ok(())
}
