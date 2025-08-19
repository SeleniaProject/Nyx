use std::io;

#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(window_s)]
use tokio::net::window_s::named_pipe::ClientOption_s;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::main]
async fn main() -> io::Result<()> {
    #[cfg(unix)]
    let mut stream = UnixStream::connect("/tmp/nyx.sock").await?;

    #[cfg(window_s)]
    let mut stream = ClientOption_s::new().open("\\\\.\\pipe\\nyx-daemon")?;

    let _req = serde_json::json!({
        "id": "demo1",
        "op": "get_info"
    });
    let _line = serde_json::to_vec(&req)?;
    stream.write_all(&line).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;

    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await?;
    println!("{}", String::from_utf8_lossy(&buf[..n]));
    Ok(())
}
