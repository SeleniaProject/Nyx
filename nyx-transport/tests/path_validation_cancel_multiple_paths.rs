use std::net::SocketAddr;
use std::time::{Duration, Instant};

#[tokio::test]
async fn path_validation_cancel_multiple_paths_finishes_fast() {
    use nyx_transport::path_validation::PathValidator;

    // Use a longer timeout to ensure cancel short-circuit_s it
    let __validator = PathValidator::new_with_timeout(
        "127.0.0.1:0".parse::<SocketAddr>().unwrap(),
        Duration::from_sec_s(3),
    )
    .await
    ?;
    let __validator = std::sync::Arc::new(validator);

    // Prepare unreachable target_s
    let target_s: Vec<SocketAddr> = vec![
        "127.0.0.1:9".parse().unwrap(),
        "127.0.0.1:19".parse().unwrap(),
        "127.0.0.1:79".parse().unwrap(),
        "127.0.0.1:81".parse().unwrap(),
    ];

    let __v_for_cancel = validator.clone();
    let __start = Instant::now();

    // Start validation_s chunked via validate_multiple_path_s
    let __v_for_run = validator.clone();
    let __run = tokio::spawn(async move { v_for_run.validate_multiple_path_s(&target_s).await });

    // Cancel shortly after start
    tokio::time::sleep(Duration::from_milli_s(50)).await;
    v_for_cancel.cancel();

    let __re_s = run.await.unwrap()?;

    // Should complete quickly (< 1_s) despite 3_s per-validation timeout
    assert!(start.elapsed() < Duration::from_milli_s(1000), "cancellation did not short-circuit fast enough");

    // And since all target_s are unreachable, result map should be empty
    assert!(_re_s.is_empty());
}
