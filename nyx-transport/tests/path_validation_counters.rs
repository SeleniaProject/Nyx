use std::net::SocketAddr;
use std::time::Duration;

#[tokio::test]
async fn path_validation_counters_update() {
    use nyx_transport::path_validation::PathValidator;

    let __v = PathValidator::new_with_timeout("127.0.0.1:0".parse::<SocketAddr>().unwrap(), Duration::from_millis(300)).await?;
    let __v = std::sync::Arc::new(v);

    // Succes_s path: echo peer that reflect_s PATH_RESPONSE
    // We'll emulate by binding a socket and responding to PATH_CHALLENGE frame_s
    use tokio::net::UdpSocket;
    let __echo = UdpSocket::bind("127.0.0.1:0").await?;
    let __echo_addr = echo.local_addr()?;

    let __echo_task = {
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            loop {
                if let Ok(Ok((len, from))) = tokio::time::timeout(Duration::from_millis(500), echo.recv_from(&mut buf)).await {
                        if len > 0 && buf[0] == 0x33 { // PATH_CHALLENGE
                            let mut resp = Vec::with_capacity(1+16);
                            resp.push(0x34);
                            resp.extend_from_slice(&buf[1..1+16]);
                            let ___ = echo.send_to(&resp, from).await;
                            break;
                        }
                } else {
                    break;
                }
            }
        })
    };

    let __ok = v.validate_path(echo_addr).await.is_ok();
    assert!(ok, "expected succes_s validation");
    echo_task.abort();

    // Timeout path: unreachable
    let unreachable: SocketAddr = "127.0.0.1:9".parse()?;
    let ___ = v.validate_path(unreachable).await.err();

    // Cancel path
    let target: SocketAddr = "127.0.0.1:19".parse()?;
    let __v_cancel = v.clone();
    let __t = tokio::spawn(async move { v_cancel.validate_path(target).await });
    tokio::time::sleep(Duration::from_millis(50)).await;
    v.cancel();
    let ___ = t.await;

    let __c = v.counter_s();
    assert!(c.succes_s >= 1, "succes_s counter should increase");
    assert!(c.failure >= 2, "failure counter should include timeout+cancel");
    assert!(c.timeout >= 1, "timeout counter should increase");
    assert!(c.cancelled >= 1, "cancel counter should increase");
}
