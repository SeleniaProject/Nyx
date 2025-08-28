use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use tokio::net::UdpSocket;

use nyx_transport::path_validation::{
    PathValidator, PATH_CHALLENGE_FRAME_TYPE, PATH_RESPONSE_FRAME_TYPE,
};

async fn bind_loopback() -> Result<UdpSocket, Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0))).await?;
    Ok(socket)
}

#[tokio::test]
async fn path_validation_success() -> Result<(), Box<dyn std::error::Error>> {
    // Validator bound to A (use ephemeral port)
    let validator = PathValidator::new_with_timeout(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        Duration::from_millis(500),
    )
    .await?;
    let a_addr = validator.local_addr()?;

    // Responder bound to B
    let b = bind_loopback().await?;
    let b_addr = b.local_addr()?;

    // Spawn responder: echo PATH_RESPONSE to A when receiving PATH_CHALLENGE
    let a_addr_clone = a_addr;
    tokio::spawn(async move {
        let mut buf = [0u8; 64];
        if let Ok((n, _from)) = b.recv_from(&mut buf).await {
            if n >= 1 && buf[0] == PATH_CHALLENGE_FRAME_TYPE {
                let mut frame = Vec::with_capacity(1 + 16);
                frame.push(PATH_RESPONSE_FRAME_TYPE);
                frame.extend_from_slice(&buf[1..(1 + 16).min(n)]);
                let _ = b.send_to(&frame, a_addr_clone).await;
            }
        }
    });

    // Validate path to B
    let metrics = validator.validate_path(b_addr).await?;
    assert!(metrics.round_trip_time > Duration::from_micros(0));
    Ok(())
}

#[tokio::test]
async fn path_validation_ignores_response_from_wrong_addr() -> Result<(), Box<dyn std::error::Error>>
{
    // Validator bound to A (ephemeral)
    let validator = PathValidator::new_with_timeout(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        Duration::from_millis(200),
    )
    .await?;
    let a_addr = validator.local_addr()?;

    // Legit target B (will NOT respond)
    let b = bind_loopback().await?;
    let b_addr = b.local_addr()?;

    // Attacker C (sends PATH_RESPONSE with copied token)
    let c = bind_loopback().await?;
    let _c_addr = c.local_addr()?;
    // Interceptor to capture token sent to B, and have attacker C reply from its own address
    tokio::spawn(async move {
        let mut buf = [0u8; 64];
        if let Ok((n, _from)) = b.recv_from(&mut buf).await {
            if n >= 1 && buf[0] == PATH_CHALLENGE_FRAME_TYPE {
                let mut frame = Vec::with_capacity(1 + 16);
                frame.push(PATH_RESPONSE_FRAME_TYPE);
                frame.extend_from_slice(&buf[1..(1 + 16).min(n)]);
                // Send forged response to A from C (wrong address)
                let _ = c.send_to(&frame, a_addr).await;
            }
        }
    });

    // Expect timeout because response came from wrong addr
    let err = validator
        .validate_path(b_addr)
        .await
        .err()
        .ok_or("Expected error")?;
    let msg = format!("{err}");
    assert!(
        msg.to_lowercase().contains("timed")
            || msg.to_lowercase().contains("no valid path_response")
    );
    Ok(())
}

#[tokio::test]
async fn path_validation_rejects_malformed_and_old_token() -> Result<(), Box<dyn std::error::Error>>
{
    // Validator bound to A with very short timeout
    let validator = PathValidator::new_with_timeout(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        Duration::from_millis(150),
    )
    .await?;
    let a_addr = validator.local_addr()?;

    // Target B
    let b = bind_loopback().await?;
    let b_addr = b.local_addr()?;

    // 1) Malformed: send PATH_RESPONSE with too short token
    let a_addr_short = a_addr;
    tokio::spawn(async move {
        let mut buf = [0u8; 64];
        if let Ok((_n, _from)) = b.recv_from(&mut buf).await {
            let frame_local = vec![PATH_RESPONSE_FRAME_TYPE, 0xAA, 0xBB]; // too short
            let _ = b.send_to(&frame_local, a_addr_short).await;
        }
    });

    // Expect timeout due to malformed response
    let _ = validator
        .validate_path(b_addr)
        .await
        .err()
        .ok_or("Expected error")?;

    // 2) Old token: send a delayed valid-looking response after timeout elapsed
    let validator2 = PathValidator::new_with_timeout(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        Duration::from_millis(100),
    )
    .await?;
    let a2_addr = validator2.local_addr()?;
    let b2 = bind_loopback().await?;
    let b2_addr = b2.local_addr()?;

    let a2_addr_clone = a2_addr;
    tokio::spawn(async move {
        let mut buf = [0u8; 64];
        if let Ok((n, _)) = b2.recv_from(&mut buf).await {
            if n >= 1 && buf[0] == PATH_CHALLENGE_FRAME_TYPE {
                // Delay beyond timeout
                tokio::time::sleep(Duration::from_millis(200)).await;
                let mut frame_local = Vec::with_capacity(1 + 16);
                frame_local.push(PATH_RESPONSE_FRAME_TYPE);
                frame_local.extend_from_slice(&buf[1..(1 + 16).min(n)]);
                let _ = b2.send_to(&frame_local, a2_addr_clone).await;
            }
        }
    });

    let _ = validator2
        .validate_path(b2_addr)
        .await
        .err()
        .ok_or("Expected error")?;
    Ok(())
}

#[tokio::test]
async fn multiple_paths_concurrent_validation_succeeds() -> Result<(), Box<dyn std::error::Error>> {
    // One validator socket (ephemeral)
    let validator = PathValidator::new_with_timeout(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        Duration::from_millis(500),
    )
    .await?;
    let a_addr = validator.local_addr()?;

    // Prepare three responder_s and addresse_s
    let r1 = bind_loopback().await?;
    let r1_addr = r1.local_addr()?;
    let r2 = bind_loopback().await?;
    let r2_addr = r2.local_addr()?;
    let r3 = bind_loopback().await?;
    let r3_addr = r3.local_addr()?;

    // Spawn responder_s that mirror PATH_RESPONSE back to validator
    let spawn_resp = |sock: UdpSocket, dst: SocketAddr| {
        tokio::spawn(async move {
            let mut buf = [0u8; 64];
            if let Ok((n, _)) = sock.recv_from(&mut buf).await {
                if n >= 1 && buf[0] == PATH_CHALLENGE_FRAME_TYPE {
                    let mut frame_local = Vec::with_capacity(17);
                    frame_local.push(PATH_RESPONSE_FRAME_TYPE);
                    frame_local.extend_from_slice(&buf[1..(1 + 16).min(n)]);
                    let _ = sock.send_to(&frame_local, dst).await;
                }
            }
        })
    };
    spawn_resp(r1, a_addr);
    spawn_resp(r2, a_addr);
    spawn_resp(r3, a_addr);

    let targets = vec![r1_addr, r2_addr, r3_addr];
    let results = validator.validate_multiple_paths(&targets).await?;
    assert_eq!(results.len(), 3);
    Ok(())
}

#[tokio::test]
async fn retry_eventual_success_on_second_attempt() -> Result<(), Box<dyn std::error::Error>> {
    // Validator with small timeout and 2 retrie_s to allow second attempt to succeed
    let validator = nyx_transport::path_validation::PathValidator::new_with_timeout_and_retries(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        Duration::from_millis(120),
        2,
    )
    .await?;
    let a_addr = validator.local_addr()?;

    // Responder that ignores first challenge and responds to the second
    let sock = bind_loopback().await?;
    let target = sock.local_addr()?;
    tokio::spawn(async move {
        let mut buf = [0u8; 64];
        // 1st receive: ignore
        let _ = sock.recv_from(&mut buf).await;
        // 2nd receive: respond
        if let Ok((n, _)) = sock.recv_from(&mut buf).await {
            if n >= 1 && buf[0] == PATH_CHALLENGE_FRAME_TYPE {
                let mut frame_local = Vec::with_capacity(17);
                frame_local.push(PATH_RESPONSE_FRAME_TYPE);
                frame_local.extend_from_slice(&buf[1..(1 + 16).min(n)]);
                let _ = sock.send_to(&frame_local, a_addr).await;
            }
        }
    });

    let metrics = validator.validate_path(target).await?;
    assert!(metrics.round_trip_time > Duration::from_micros(0));
    Ok(())
}
