//! Records a real client's NetherNet join for diffing against ours: a logging relay in front of a
//! local BDS (`transport=nethernet`).
//! `cargo run -p acacia-client --example nn_record -- [listen_port=19170] [bds=127.0.0.1:19160] [out=.testserver/nn-capture] [bind_ip=127.0.0.1]`
//! - TCP `listen_port`: signaling, passed through (HTTP or TLS) and logged to `tcp.log` as received.
//!   The answer's candidates are rewritten to the UDP relay and the offer's to an unreachable
//!   address, so all UDP goes through the relay; identities sign only the fingerprints, so both
//!   sides still accept the SDP.
//! - UDP `listen_port + 1`: relays ICE/DTLS/SCTP to BDS and logs each datagram as hex to `udp.log`.
//! Join `<bind_ip>:<listen_port>` from the game (a LAN IP avoids app loopback isolation), then stop with Ctrl+C.
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Mutex;

type Log = Arc<Mutex<File>>;
type Target = Arc<Mutex<Option<SocketAddr>>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let port: u16 = args.next().map_or(19170, |p| p.parse().expect("port"));
    let bds: SocketAddr = args.next().unwrap_or_else(|| "127.0.0.1:19160".into()).parse()?;
    let out = args.next().unwrap_or_else(|| ".testserver/nn-capture".into());
    let ip: std::net::IpAddr = args.next().unwrap_or_else(|| "127.0.0.1".into()).parse()?;
    tokio::fs::create_dir_all(&out).await?;
    let (tcp_log, udp_log) = (open_log(&out, "tcp.log").await?, open_log(&out, "udp.log").await?);
    let start = Instant::now();

    let relay = Arc::new(UdpSocket::bind((ip, port + 1)).await?);
    let upstream = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
    let client: Arc<Mutex<Option<SocketAddr>>> = Arc::default();
    // BDS's WebRTC listens per interface, not on loopback: relay to the candidate it advertised.
    let target: Target = Arc::default();
    tokio::spawn(pump_udp(relay.clone(), upstream.clone(), client.clone(), target.clone(), udp_log.clone(), start));

    let listener = TcpListener::bind((ip, port)).await?;
    println!("signaling on {ip}:{port}, UDP relay on {}, logging to {out}", relay.local_addr()?);
    loop {
        let (stream, from) = listener.accept().await?;
        let relay_addr = relay.local_addr()?;
        let (log, target) = (tcp_log.clone(), target.clone());
        tokio::spawn(async move {
            if let Err(e) = proxy_tcp(stream, from, bds, relay_addr, &target, &log, start).await {
                println!("tcp {from}: {e}");
            }
        });
    }
}

async fn open_log(dir: &str, name: &str) -> std::io::Result<Log> {
    let file = OpenOptions::new().create(true).append(true).open(format!("{dir}/{name}")).await?;
    Ok(Arc::new(Mutex::new(file)))
}

async fn record(log: &Log, start: Instant, line: String) {
    let line = format!("[{:>8.3}] {line}\n", start.elapsed().as_secs_f64());
    print!("{line}");
    let _ = log.lock().await.write_all(line.as_bytes()).await;
}

/// One signaling connection: read the client's bytes until it pauses for a reply, forward them, then
/// relay the server's response until close. Servers close after each response.
async fn proxy_tcp(mut client: TcpStream, from: SocketAddr, bds: SocketAddr, relay: SocketAddr, target: &Target, log: &Log, start: Instant) -> std::io::Result<()> {
    let mut server = TcpStream::connect(bds).await?;
    let mut request = read_request(&mut client).await?;
    record(log, start, format!("TCP {from} -> server {} bytes\n{}", request.len(), render(&request))).await;
    if request.starts_with(b"POST /v1/join/") {
        // An unreachable client candidate (TEST-NET-1) leaves the server only the relayed path,
        // so the whole DTLS handshake passes through the UDP log.
        request = rewrite_candidates(&request, "192.0.2.1:9".parse().expect("literal")).0;
    }
    server.write_all(&request).await?;
    let mut response = Vec::new();
    let _ = server.read_to_end(&mut response).await;
    let is_join_post = request.starts_with(b"POST /v1/join/");
    if is_join_post {
        let (rewritten, advertised) = rewrite_candidates(&response, relay);
        response = rewritten;
        *target.lock().await = advertised;
    }
    record(log, start, format!("TCP server -> {from} {} bytes{}\n{}", response.len(), if is_join_post { " (candidates rewritten)" } else { "" }, render(&response))).await;
    client.write_all(&response).await?;
    client.shutdown().await
}

/// HTTP: headers plus Content-Length body. Anything else (a TLS ClientHello): what the first read returns.
async fn read_request(stream: &mut TcpStream) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 16384];
    loop {
        let n = stream.read(&mut chunk).await?;
        buf.extend_from_slice(&chunk[..n]);
        if n == 0 || !buf.first().is_some_and(u8::is_ascii_uppercase) {
            return Ok(buf);
        }
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..end]).to_ascii_lowercase();
            let len: usize = head.lines().find_map(|l| l.strip_prefix("content-length:")).map_or(0, |v| v.trim().parse().unwrap_or(0));
            if buf.len() >= end + 4 + len {
                return Ok(buf);
            }
        }
    }
}

/// Replaces an offer's or answer's candidates (and m=/c= defaults) with `relay`, fixing
/// Content-Length. Also returns the first IPv4 candidate it replaced.
fn rewrite_candidates(response: &[u8], relay: SocketAddr) -> (Vec<u8>, Option<SocketAddr>) {
    let Some(split) = response.windows(4).position(|w| w == b"\r\n\r\n") else { return (response.to_vec(), None) };
    let head = String::from_utf8_lossy(&response[..split]);
    let body = String::from_utf8_lossy(&response[split + 4..]);
    let mut out_body = String::new();
    let mut replaced = false;
    let mut advertised = None;
    for line in body.lines() {
        let line = if line.starts_with("a=candidate:") {
            // `candidate:<foundation> <component> udp <priority> <ip> <port> typ ...`
            let fields: Vec<&str> = line.split(' ').collect();
            if let (None, Some(ip), Some(port)) = (advertised, fields.get(4), fields.get(5)) {
                advertised = format!("{ip}:{port}").parse::<SocketAddr>().ok().filter(SocketAddr::is_ipv4);
            }
            if replaced {
                continue;
            }
            replaced = true;
            format!("a=candidate:1 1 udp 2122260223 {} {} typ host generation 0 network-id 1 network-cost 10", relay.ip(), relay.port())
        } else if let Some(rest) = line.strip_prefix("m=application ") {
            format!("m=application {} {}", relay.port(), rest.split_once(' ').map_or(rest, |(_, r)| r))
        } else if line.starts_with("c=IN ") {
            format!("c=IN IP4 {}", relay.ip())
        } else {
            line.to_owned()
        };
        out_body.push_str(&line);
        out_body.push_str("\r\n");
    }
    let head: String = head
        .lines()
        .map(|l| if l.to_ascii_lowercase().starts_with("content-length:") { format!("Content-Length: {}", out_body.len()) } else { l.to_owned() })
        .collect::<Vec<_>>()
        .join("\r\n");
    (format!("{head}\r\n\r\n{out_body}").into_bytes(), advertised)
}

async fn pump_udp(relay: Arc<UdpSocket>, upstream: Arc<UdpSocket>, client: Target, target: Target, log: Log, start: Instant) {
    let (mut a, mut b) = (vec![0u8; 65536], vec![0u8; 65536]);
    let mut seen = 0usize;
    // Full hex for the handshake; afterwards only a prefix, to keep the log readable.
    let mut sample = |d: &[u8]| {
        seen += 1;
        hex(if seen <= 80 { d } else { &d[..d.len().min(16)] })
    };
    loop {
        // Errors are matched, not pattern-skipped: Windows reports ICMP port-unreachable from an
        // earlier send as a recv error, and a select! with every branch disabled panics.
        tokio::select! {
            r = relay.recv_from(&mut a) => match r {
                Ok((n, from)) => {
                    *client.lock().await = Some(from);
                    record(&log, start, format!("UDP C->S {n:>5} {}", sample(&a[..n]))).await;
                    if let Some(to) = *target.lock().await {
                        let _ = upstream.send_to(&a[..n], to).await;
                    }
                }
                Err(e) => record(&log, start, format!("UDP relay recv error (ignored): {e}")).await,
            },
            r = upstream.recv_from(&mut b) => match r {
                Ok((n, _)) => {
                    record(&log, start, format!("UDP S->C {n:>5} {}", sample(&b[..n]))).await;
                    if let Some(to) = *client.lock().await {
                        let _ = relay.send_to(&b[..n], to).await;
                    }
                }
                Err(e) => record(&log, start, format!("UDP upstream recv error (ignored): {e}")).await,
            },
        }
    }
}

fn render(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => format!("(binary) {}", hex(bytes)),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
