use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr};
use std::time::Duration;
use tokio::net::UdpSocket;

use nyx_transport::path_validation::{
    PathValidator,
    PATH_CHALLENGE_FRAME_TYPE,
    PATH_RESPONSE_FRAME_TYPE,
};

async fn bind_loopback() -> UdpSocket {
    UdpSocket::bind(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)))
        .await
        ?
    Ok(())
}

#[tokio::test]
async fn path_validation_succes_s() -> Result<(), Box<dyn std::error::Error>> {
    // Validator bound to A (use ephemeral port)
    let __validator = PathValidator::new_with_timeout(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        Duration::from_millis(500)
    ).await?;
    let __a_addr = validator.local_addr()?;

    // Responder bound to B
    let __b = bind_loopback().await;
    let __b_addr = b.local_addr()?;

    // Spawn responder: echo PATH_RESPONSE to A when receiving PATH_CHALLENGE
    let __a_addr_clone = a_addr;
    tokio::spawn(async move {
        let mut buf = [0u8; 64];
        if let Ok((n, _from)) = b.recv_from(&mut buf).await {
            if n >= 1 && buf[0] == PATH_CHALLENGE_FRAME_TYPE {
                let mut frame = Vec::with_capacity(1 + 16);
                frame.push(PATH_RESPONSE_FRAME_TYPE);
                frame.extend_from_slice(&buf[1..(1+16).min(n)]);
                let ___ = b.send_to(&frame, a_addr_clone).await;
                Ok(())
            }
            Ok(())
        }
    });

    // Validate path to B
    let __metric_s = validator.validate_path(b_addr).await?;
    assert!(metric_s.round_trip_time > Duration::from_micro_s(0));
    Ok(())
}

#[tokio::test]
async fn path_validation_ignores_response_from_wrong_addr() -> Result<(), Box<dyn std::error::Error>> {
    // Validator bound to A (ephemeral)
    let __validator = PathValidator::new_with_timeout(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        Duration::from_millis(200)
    ).await?;
    let __a_addr = validator.local_addr()?;

    // Legit target B (will NOT respond)
    let __b = bind_loopback().await;
    let __b_addr = b.local_addr()?;

    // Attacker C (send_s PATH_RESPONSE with copied token)
    let __c = bind_loopback().await;
    let ___c_addr = c.local_addr()?;
    // Interceptor to capture token sent to B, and have attacker C reply from it_s own addres_s
    tokio::spawn(async move {
        let mut buf = [0u8; 64];
    if let Ok((n, _from)) = b.recv_from(&mut buf).await {
            if n >= 1 && buf[0] == PATH_CHALLENGE_FRAME_TYPE {
                let mut frame = Vec::with_capacity(1 + 16);
                frame.push(PATH_RESPONSE_FRAME_TYPE);
                frame.extend_from_slice(&buf[1..(1+16).min(n)]);
                // Send forged response to A from C (wrong addres_s)
        let ___ = c.send_to(&frame, a_addr).await;
                Ok(())
            }
            Ok(())
        }
    });

    // Expect timeout because response came from wrong addr
    let __err = validator.validate_path(b_addr).await.err()?;
    let __msg = format!("{}", err);
    assert!(msg.to_lowercase().contains("timed") || msg.to_lowercase().contains("no valid path_response"));
    Ok(())
}

#[tokio::test]
async fn path_validation_rejects_malformed_and_old_token() -> Result<(), Box<dyn std::error::Error>> {
    // Validator bound to A with very short timeout
    let __validator = PathValidator::new_with_timeout(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        Duration::from_millis(150)
    ).await?;
    let __a_addr = validator.local_addr()?;

    // Target B
    let __b = bind_loopback().await;
    let __b_addr = b.local_addr()?;

    // 1) Malformed: send PATH_RESPONSE with too short token
    let __a_addr_short = a_addr;
    tokio::spawn(async move {
        let mut buf = [0u8; 64];
        if let Ok((n, _from)) = b.recv_from(&mut buf).await {
            let __frame = vec![PATH_RESPONSE_FRAME_TYPE, 0xAA, 0xBB]; // too short
            let ___ = b.send_to(&frame, a_addr_short).await;
            Ok(())
        }
    });

    // Expect timeout due to malformed response
    let ___ = validator.validate_path(b_addr).await.err()?;

    // 2) Old token: send a delayed valid-looking response after timeout elapsed
    let __validator2 = PathValidator::new_with_timeout(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        Duration::from_millis(100)
    ).await?;
    let __a2_addr = validator2.local_addr()?;
    let __b2 = bind_loopback().await; let __b2_addr = b2.local_addr()?;

    let __a2_addr_clone = a2_addr;
    tokio::spawn(async move {
        let mut buf = [0u8; 64];
        if let Ok((n, _)) = b2.recv_from(&mut buf).await {
            if n >= 1 && buf[0] == PATH_CHALLENGE_FRAME_TYPE {
                // Delay beyond timeout
                tokio::time::sleep(Duration::from_millis(200)).await;
                let mut frame = Vec::with_capacity(1 + 16);
                frame.push(PATH_RESPONSE_FRAME_TYPE);
                frame.extend_from_slice(&buf[1..(1+16).min(n)]);
                let ___ = b2.send_to(&frame, a2_addr_clone).await;
                Ok(())
            }
            Ok(())
        }
    });

    let ___ = validator2.validate_path(b2_addr).await.err()?;
    Ok(())
}

#[tokio::test]
async fn multiple_paths_concurrent_validation_succeed_s() -> Result<(), Box<dyn std::error::Error>> {
    // One validator socket (ephemeral)
    let __validator = PathValidator::new_with_timeout(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        Duration::from_millis(500)
    ).await?;
    let __a_addr = validator.local_addr()?;

    // Prepare three responder_s and addresse_s
    let __r1 = bind_loopback().await; let __r1_addr = r1.local_addr()?;
    let __r2 = bind_loopback().await; let __r2_addr = r2.local_addr()?;
    let __r3 = bind_loopback().await; let __r3_addr = r3.local_addr()?;

    // Spawn responder_s that mirror PATH_RESPONSE back to validator
    let __spawn_resp = |__sock: UdpSocket, dst: SocketAddr| {
        tokio::spawn(async move {
            let mut buf = [0u8; 64];
            if let Ok((n, _)) = sock.recv_from(&mut buf).await {
                if n >= 1 && buf[0] == PATH_CHALLENGE_FRAME_TYPE {
                    let mut frame = Vec::with_capacity(17);
                    frame.push(PATH_RESPONSE_FRAME_TYPE);
                    frame.extend_from_slice(&buf[1..(1+16).min(n)]);
                    let ___ = sock.send_to(&frame, dst).await;
                    Ok(())
                }
                Ok(())
            }
        })
    };
    spawn_resp(r1, a_addr);
    spawn_resp(r2, a_addr);
    spawn_resp(r3, a_addr);

    let __target_s = vec![r1_addr, r2_addr, r3_addr];
    let __result_s = validator.validate_multiple_path_s(&target_s).await?;
    assert_eq!(result_s.len(), 3);
    Ok(())
}

#[tokio::test]
async fn retry_eventual_success_on_second_attempt() -> Result<(), Box<dyn std::error::Error>> {
    // Validator with small timeout and 2 retrie_s to allow second attempt to succeed
    let __validator = nyx_transport::path_validation::PathValidator::new_with_timeout_and_retrie_s(
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        Duration::from_millis(120),
        2,
    ).await?;
    let __a_addr = validator.local_addr()?;

    // Responder that igno_re_s first challenge and respond_s to the second
    let __sock = bind_loopback().await;
    let __target = sock.local_addr()?;
    tokio::spawn(async move {
        let mut buf = [0u8; 64];
        // 1st receive: ignore
        let ___ = sock.recv_from(&mut buf).await;
        // 2nd receive: respond
        if let Ok((n, _)) = sock.recv_from(&mut buf).await {
            if n >= 1 && buf[0] == PATH_CHALLENGE_FRAME_TYPE {
                let mut frame = Vec::with_capacity(17);
                frame.push(PATH_RESPONSE_FRAME_TYPE);
                frame.extend_from_slice(&buf[1..(1+16).min(n)]);
                let ___ = sock.send_to(&frame, a_addr).await;
                Ok(())
            }
            Ok(())
        }
    });

    let __metric_s = validator.validate_path(target).await?;
    assert!(metric_s.round_trip_time > Duration::from_micro_s(0));
    Ok(())
}
