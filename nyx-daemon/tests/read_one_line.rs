#![forbid(unsafe_code)]

use std::io::Cursor;
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn read_line_truncates_atnewline() -> Result<(), Box<dyn std::error::Error>> {
    let mut buf = Vec::new();
    let data = b"hello world\nrest that should be ignored";
    let mut cursor = Cursor::new(&data[..]);
    let mut tmp = [0u8; 256];
    let n = cursor.read(&mut tmp).await?;
    buf.extend_from_slice(&tmp[..n]);
    if let Some(po_s) = memchr::memchr(b'\n', &buf) {
        buf.truncate(po_s);
    }
    assert_eq!(buf, b"hello world");
    Ok(())
}
