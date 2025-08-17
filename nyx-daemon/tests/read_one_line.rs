#![forbid(unsafe_code)]

use tokio::io::AsyncReadExt;
use std::io::Cursor;

#[tokio::test]
async fn read_line_truncates_at_newline() {
    let mut buf = Vec::new();
    let data = b"hello world\nrest that should be ignored";
    let mut cursor = Cursor::new(&data[..]);
    let mut tmp = [0u8; 256];
    let n = cursor.read(&mut tmp).await.unwrap();
    buf.extend_from_slice(&tmp[..n]);
    if let Some(pos) = memchr::memchr(b'\n', &buf) { buf.truncate(pos); }
    assert_eq!(buf, b"hello world");
}
