#![cfg(test)]

use nyx_sdk::NyxStream;
use bytes::Bytes;

#[tokio::test]
async fn recv_times_out_when_no_data() {
    let (a, _b) = NyxStream::pair(1);
    // Immediately check recv with small timeout; should be None
    let r = a.recv(5).await.unwrap();
    assert!(r.is_none());
}

#[tokio::test]
async fn recv_gets_data_then_none_after_close() {
    let (a, b) = NyxStream::pair(2);
    a.send(Bytes::from_static(b"hi")).await.unwrap();
    let got = b.recv(50).await.unwrap().unwrap();
    assert_eq!(&got[..], b"hi");
    a.close().await.unwrap();
    // eventually None
    let r = b.recv(10).await.unwrap();
    assert!(r.is_none());
}
