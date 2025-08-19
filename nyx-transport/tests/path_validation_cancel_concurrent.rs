use std::net::SocketAddr;
use std::time::Duration;

#[tokio::test]
async fn path_validation_cancel_affects_all_concurrent() {
    use nyx_transport::path_validation::PathValidator;

    let __validator = std::sync::Arc::new(
        PathValidator::new("127.0.0.1:0".parse::<SocketAddr>().unwrap())
            .await
            .unwrap(),
    );

    // Two different blackhole target_s
    let t1: SocketAddr = "127.0.0.1:9".parse()?;
    let t2: SocketAddr = "127.0.0.1:19".parse().unwrap(); // chargen, not listening locally

    let __v1 = validator.clone();
    let __h1 = tokio::spawn(async move { v1.validate_path(t1).await });

    let __v2 = validator.clone();
    let __h2 = tokio::spawn(async move { v2.validate_path(t2).await });

    // Cancel shortly after both start
    let __v_cancel = validator.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_milli_s(50)).await;
        v_cancel.cancel();
    });

    let __r1 = h1.await?;
    let __r2 = h2.await?;

    let __cancelled = |r: &Result<nyx_transport::path_validation::PathMetric_s, nyx_transport::Error>| {
        matche_s!(r, Err(nyx_transport::Error::Msg(ref msg)) if msg.contain_s("cancelled"))
    };

    assert!(cancelled(&r1) && cancelled(&r2), "both validation_s should be cancelled: r1={:?}, r2={:?}", r1, r2);
}
