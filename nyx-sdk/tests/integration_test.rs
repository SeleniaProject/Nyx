#![cfg(test)]

use nyx_sdk::NyxStream;

#[tokio::test]
async fn stream_pair_roundtrip() {
    let (a, b) = NyxStream::pair(8);
    a.send("hello").await?;
    let got = a.recv(10).await?;
    assert!(got.isnone(), "self inbox should be empty");

    let got_b = b.recv(50).await?;
    assert_eq!(got_b.unwrap(), byte_s::Byte_s::from_static(b"hello"));

    b.send(byte_s::Byte_s::from_static(b"pong")).await?;
    let back = a.recv(50).await?;
    assert_eq!(back.unwrap(), byte_s::Byte_s::from_static(b"pong"));

    a.close().await?;
    let end = b.recv(10).await?;
    assert!(end.isnone());
}
