mod integrated_frame_processor_tests {
    use crate::integrated_frame_processor::{
        FrameProcessingEvent, IntegratedFrameConfig, IntegratedFrameProcessor,
    };
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use tokio::time::sleep;

    fn build_frame_bytes(stream_id: u32, offset: u32, fin: bool, data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&stream_id.to_be_bytes());
        v.extend_from_slice(&offset.to_be_bytes());
        v.push(if fin { 1 } else { 0 });
        v.extend_from_slice(&(data.len() as u32).to_be_bytes());
        v.extend_from_slice(data);
        v
    }

    #[tokio::test]
    async fn test_integrated_processor_basic_functionality() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let config = IntegratedFrameConfig::default();
        let processor = IntegratedFrameProcessor::new(config, tx);
        processor.start().await.unwrap();

        let frame = build_frame_bytes(1, 0, false, b"data");
        let res = processor.process_frame(&frame).await;
        assert!(res.is_ok());

        // Stream should be visible via public API
        let info = processor.get_stream_info(1).await;
        assert!(info.is_some());

        processor.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_stream_context_management() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let config = IntegratedFrameConfig::default();
        let processor = IntegratedFrameProcessor::new(config, tx);
        processor.start().await.unwrap();

        let stream_id = 42;
        let frame = build_frame_bytes(stream_id, 0, false, b"xyz");
        processor.process_frame(&frame).await.unwrap();

        // Verify context via API
        let info = processor.get_stream_info(stream_id).await;
        assert!(info.is_some());

        // Close stream
        processor
            .close_stream(stream_id, "test".into())
            .await
            .unwrap();
        let info2 = processor.get_stream_info(stream_id).await;
        assert!(info2.is_none());

        processor.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_statistics_collection() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut config = IntegratedFrameConfig::default();
        config.flow_control_update_interval = Duration::from_millis(50);
        let processor = IntegratedFrameProcessor::new(config, tx);
        processor.start().await.unwrap();

        // Send 10 frames to distinct streams
        for i in 0..10u32 {
            let frame = build_frame_bytes(i, 0, false, &[0x55; 100]);
            processor.process_frame(&frame).await.unwrap();
        }

        sleep(Duration::from_millis(120)).await;
        let stats = processor.get_stats().await;
        assert_eq!(stats.total_frames_processed, 10);
        assert_eq!(stats.active_streams, 10);
        assert!(stats.total_bytes_processed > 0);

        processor.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_event_handling() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let processor = Arc::new(IntegratedFrameProcessor::new(
            IntegratedFrameConfig::default(),
            tx,
        ));
        processor.start().await.unwrap();

        let frame = build_frame_bytes(1, 0, false, b"event");
        let p = Arc::clone(&processor);
        let frame_clone = frame.clone();
        tokio::spawn(async move {
            sleep(Duration::from_millis(10)).await;
            p.process_frame(&frame_clone).await.unwrap();
        });

        let evt = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await;
        assert!(evt.is_ok());
        if let Some(ev) = evt.unwrap() {
            match ev {
                FrameProcessingEvent::FrameReassembled { stream_id, .. } => {
                    assert_eq!(stream_id, 1)
                }
                _ => {}
            }
        }

        processor.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_concurrent_stream_processing() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut cfg = IntegratedFrameConfig::default();
        cfg.max_concurrent_streams = 50;
        let processor: Arc<IntegratedFrameProcessor> =
            Arc::new(IntegratedFrameProcessor::new(cfg, tx));
        processor.start().await.unwrap();

        let mut handles = Vec::new();
        for i in 0..20u32 {
            let p = Arc::clone(&processor);
            handles.push(tokio::spawn(async move {
                for j in 0..5u32 {
                    let sid = i * 5 + j;
                    let frame = build_frame_bytes(sid, 0, false, &[0x88; 75]);
                    let _ = p.process_frame(&frame).await; // ignore capacity errors for high ids
                }
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        // Allow background stats updater to run at least once
        sleep(Duration::from_millis(200)).await;
        let stats = processor.get_stats().await;
        assert!(stats.active_streams >= 10);
        assert!(stats.total_frames_processed >= 50);

        processor.shutdown().await.unwrap();
    }
}
