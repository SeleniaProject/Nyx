fn main() -> Result<(), Box<dyn std::error::Error>> {
    use nyx_transport::UdpEndpoint;
    let a = UdpEndpoint::bind_loopback()?;
    let b = UdpEndpoint::bind_loopback()?;
    let msg = b"hello";
    a.send_to(msg, b.local_addr())?;
    let mut buf = [0u8; 16];
    let (n, from) = b.recv_from(&mut buf)?;
    println!("got {} bytes from {}: {:?}", n, from, &buf[..n]);
    Ok(())
}
