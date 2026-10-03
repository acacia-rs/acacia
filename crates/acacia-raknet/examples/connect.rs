//! Smoke test: connects, sends a Bedrock RequestNetworkSettings, prints what comes back, then closes.
//! `cargo run -p acacia-raknet --example connect -- 127.0.0.1:19140`
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use acacia_raknet::{Client, Config, Event, Reliability};
use bytes::Bytes;
use tokio::net::UdpSocket;

/// Batch holding RequestNetworkSettings (id 193) for protocol 2193.
const REQUEST_NETWORK_SETTINGS: &[u8] = &[0xfe, 0x06, 0xc1, 0x01, 0x00, 0x00, 0x08, 0x91];

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    let addr: SocketAddr = std::env::args().nth(1).as_deref().unwrap_or("127.0.0.1:19140").parse().expect("server address");
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.connect(addr).await?;

    let start = Instant::now();
    let guid = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64 ^ u64::from(std::process::id());
    let mut client = Client::new(Config::new(guid), addr, start);
    let mut buf = vec![0u8; 2048];
    loop {
        let now = Instant::now();
        while let Some(d) = client.poll_transmit(now) {
            sock.send(&d).await?;
        }
        while let Some(event) = client.poll_event() {
            match event {
                Event::Connected { mtu } => {
                    println!("connected in {:?} (mtu {mtu})", start.elapsed());
                    client.send(Bytes::from_static(REQUEST_NETWORK_SETTINGS), Reliability::ReliableOrdered);
                }
                Event::Message(m) => println!("message ({} bytes): {:02x?}", m.len(), &m[..m.len().min(24)]),
                Event::Disconnected(reason) => {
                    println!("disconnected after {:?}: {reason:?} (rtt {:?})", start.elapsed(), client.rtt());
                    return Ok(());
                }
            }
        }
        if client.is_connected() && start.elapsed() > Duration::from_secs(3) {
            client.close(Instant::now());
            continue;
        }
        let deadline = client.poll_timeout().unwrap_or(now + Duration::from_secs(1));
        tokio::select! {
            r = sock.recv(&mut buf) => match r {
                Ok(n) => client.handle_datagram(Instant::now(), Bytes::copy_from_slice(&buf[..n])),
                // Windows reports ICMP port-unreachable as a reset on connected UDP sockets.
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => {}
                Err(e) => return Err(e),
            },
            _ = tokio::time::sleep_until(deadline.into()) => client.handle_timeout(Instant::now()),
        }
    }
}
