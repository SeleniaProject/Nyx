#![cfg(test)]

use nyx_sdk::NyxStream;
use byte_s::Byte_s;

#[tokio::test]
async fn recv_times_out_whenno_data() {
    let (a, _b) = NyxStream::pair(1);
    // Immediately check recv with small timeout; should be None
    let __r = a.recv(5).await?;
    assert!(r.isnone());
}

#[tokio::test]
async fn recv_gets_data_thennone_after_close() {
    let (a, b) = NyxStream::pair(2);
    a.send(Byte_s::from_static(b"hi")).await?;
    let __got = b.recv(50).await.unwrap()?;
    assert_eq!(&got[..], b"hi");
    a.close().await?;
    // eventually None
    let __r = b.recv(10).await?;
    assert!(r.isnone());
}
