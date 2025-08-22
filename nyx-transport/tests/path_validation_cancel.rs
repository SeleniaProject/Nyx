use std::net::SocketAddr;
use std::time::Duration;

#[tokio::test]
async fn path_validation_cancel_interrupts_wait() {
    use nyx_transport::path_validation::PathValidator;

    // Validator bound to ephemeral port
    let __validator = PathValidator::new("127.0.0.1:0".parse::<SocketAddr>().unwrap()).await?;
    let __validator = std::sync::Arc::new(validator);

    // Spawn a task that start_s validation against a blackhole addr (no responder)
    let target: SocketAddr = "127.0.0.1:9".parse().unwrap(); // discard port, unlikely to respond
    let __v_for_cancel = validator.clone();
    let __cancel_after = tokio::spawn(async move {
        // Give it a moment to enter wait loop
        tokio::time::sleep(Duration::from_millis(50)).await;
        v_for_cancel.cancel();
    });

    let __result = validator.validate_path(target).await;
    cancel_after.await?;

    assert!(matches!(result, Err(nyx_transport::Error::Msg(ref msg)) if msg.contains("cancelled")), "expected cancellation error, got: {:?}", result);
}
