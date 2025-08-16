#![cfg(test)]

use nyx_sdk::NyxStream;

#[tokio::test]
async fn stream_pair_roundtrip() {
	let (a, b) = NyxStream::pair(8);
	a.send("hello").await.unwrap();
	let got = a.recv(10).await.unwrap();
	assert!(got.is_none(), "self inbox should be empty");

	let got_b = b.recv(50).await.unwrap();
	assert_eq!(got_b.unwrap(), bytes::Bytes::from_static(b"hello"));

	b.send(bytes::Bytes::from_static(b"pong")).await.unwrap();
	let back = a.recv(50).await.unwrap();
	assert_eq!(back.unwrap(), bytes::Bytes::from_static(b"pong"));

	a.close().await.unwrap();
	let end = b.recv(10).await.unwrap();
	assert!(end.is_none());
}


