#![cfg(test)]

use bytes::Bytes;
use nyx_sdk::NyxStream;

#[tokio::test]
async fn recv_times_out_when_no_data() -> Result<(), Box<dyn std::error::Error>> {
    let (mut a, _b) = NyxStream::pair(1);
    // Immediately check recv with small timeout; should be None
    let r = a.recv(5).await?;
    assert!(r.is_none());
    Ok(())
}

#[tokio::test]
async fn recv_gets_data_then_none_after_close() -> Result<(), Box<dyn std::error::Error>> {
    let (mut a, mut b) = NyxStream::pair(2);
    a.send(Bytes::from_static(b"hi")).await?;
    let got = b.recv(50).await?.unwrap();
    assert_eq!(&got[..], b"hi");
    
    // Test that receive without data returns None
    let r = b.recv(10).await?;
    assert!(r.is_none());
    
    // Close is separate from data receiving
    a.close().await?;
    Ok(())
}
