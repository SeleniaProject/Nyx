//! Telemetry Dispatch Counters Tests
//!
//! Integration tests for telemetry functionality - only run with feature = "telemetry"

#[cfg(feature = "telemetry")]
use nyx_stream::telemetry::{CounterType, MetricEvent, TelemetryDispatcher};

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn test_telemetry_counter_dispatch() {
    let dispatcher = TelemetryDispatcher::new();

    // Start the dispatcher
    dispatcher
        .start()
        .await
        .expect("Failed to start telemetry dispatcher");

    // Send some counter events
    let events = vec![
        MetricEvent::Counter(CounterType::PacketsReceived, 1),
        MetricEvent::Counter(CounterType::PacketsSent, 1),
        MetricEvent::Counter(CounterType::BytesReceived, 1024),
        MetricEvent::Counter(CounterType::BytesSent, 512),
    ];

    for event in events {
        dispatcher
            .dispatch(event)
            .await
            .expect("Failed to dispatch event");
    }

    // Allow some time for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify counters were updated
    let counters = dispatcher.get_counters().await;

    assert_eq!(counters.packets_received, 1);
    assert_eq!(counters.packets_sent, 1);
    assert_eq!(counters.bytes_received, 1024);
    assert_eq!(counters.bytes_sent, 512);

    // Stop the dispatcher
    dispatcher
        .stop()
        .await
        .expect("Failed to stop telemetry dispatcher");

    println!("Telemetry counter dispatch test completed");
}

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn test_telemetry_batch_dispatch() {
    let dispatcher = TelemetryDispatcher::new();
    dispatcher
        .start()
        .await
        .expect("Failed to start telemetry dispatcher");

    // Send multiple events rapidly
    for i in 0..100 {
        let event = MetricEvent::Counter(CounterType::PacketsReceived, 1);
        dispatcher
            .dispatch(event)
            .await
            .expect("Failed to dispatch event");
    }

    // Allow processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let counters = dispatcher.get_counters().await;
    assert_eq!(counters.packets_received, 100);

    dispatcher
        .stop()
        .await
        .expect("Failed to stop telemetry dispatcher");

    println!("Telemetry batch dispatch test completed");
}

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn test_telemetry_concurrent_dispatch() {
    let dispatcher = TelemetryDispatcher::new();
    dispatcher
        .start()
        .await
        .expect("Failed to start telemetry dispatcher");

    // Spawn multiple tasks that send events concurrently
    let mut handles = vec![];

    for i in 0..10 {
        let dispatcher_clone = dispatcher.clone();
        let handle = tokio::spawn(async move {
            for j in 0..10 {
                let event = MetricEvent::Counter(CounterType::BytesReceived, 1);
                dispatcher_clone
                    .dispatch(event)
                    .await
                    .expect("Failed to dispatch event");
            }
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await.expect("Task should complete successfully");
    }

    // Allow processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let counters = dispatcher.get_counters().await;
    assert_eq!(counters.bytes_received, 100); // 10 tasks * 10 events each

    dispatcher
        .stop()
        .await
        .expect("Failed to stop telemetry dispatcher");

    println!("Telemetry concurrent dispatch test completed");
}

#[cfg(not(feature = "telemetry"))]
#[test]
fn test_telemetry_disabled() {
    // When telemetry feature is disabled, this test just passes
    println!("Telemetry feature is disabled - skipping telemetry tests");
}

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn test_telemetry_error_handling() {
    let dispatcher = TelemetryDispatcher::new();
    dispatcher
        .start()
        .await
        .expect("Failed to start telemetry dispatcher");

    // Test various edge cases

    // Send event with zero value
    let zero_event = MetricEvent::Counter(CounterType::PacketsReceived, 0);
    let result = dispatcher.dispatch(zero_event).await;
    assert!(
        result.is_ok(),
        "Zero value events should be handled gracefully"
    );

    // Send event with large value
    let large_event = MetricEvent::Counter(CounterType::BytesReceived, u64::MAX);
    let result = dispatcher.dispatch(large_event).await;
    assert!(
        result.is_ok(),
        "Large value events should be handled gracefully"
    );

    dispatcher
        .stop()
        .await
        .expect("Failed to stop telemetry dispatcher");

    println!("Telemetry error handling test completed");
}

#[cfg(feature = "telemetry")]
#[tokio::test]
async fn test_telemetry_lifecycle() {
    let dispatcher = TelemetryDispatcher::new();

    // Test multiple start/stop cycles
    for i in 0..3 {
        dispatcher
            .start()
            .await
            .expect("Failed to start telemetry dispatcher");

        // Send some events
        for j in 0..5 {
            let event = MetricEvent::Counter(CounterType::PacketsSent, 1);
            dispatcher
                .dispatch(event)
                .await
                .expect("Failed to dispatch event");
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        dispatcher
            .stop()
            .await
            .expect("Failed to stop telemetry dispatcher");

        // Brief pause between cycles
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    println!("Telemetry lifecycle test completed");
}
