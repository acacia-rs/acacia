//! SOCKS5 UDP ASSOCIATE and CONNECT (RFC 1928) with optional username/password auth (RFC 1929).

mod proxy;
#[cfg(test)]
mod tests;

use std::io::{self, Error, ErrorKind};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};

pub use proxy::{ProxyParseError, Socks5Proxy};

/// A UDP association through a SOCKS5 proxy. `send` relays to the default target.
pub(crate) struct Socks5Udp {
    /// The association lives only as long as this control connection stays open.
    _control: TcpStream,
    socket: UdpSocket,
    target: SocketAddr,
}

fn proto_err(msg: &'static str) -> Error {
    Error::new(ErrorKind::InvalidData, msg)
}

/// Opens the control connection and completes method negotiation and auth.
async fn handshake(proxy: &Socks5Proxy) -> io::Result<TcpStream> {
    let mut tcp = TcpStream::connect((proxy.host.as_str(), proxy.port)).await?;
    tcp.set_nodelay(true)?;
    let methods: &[u8] = if proxy.credentials.is_some() { &[5, 2, 0, 2] } else { &[5, 1, 0] };
    tcp.write_all(methods).await?;
    let mut choice = [0u8; 2];
    tcp.read_exact(&mut choice).await?;
    match choice {
        [5, 0] => Ok(tcp),
        [5, 2] => authenticate(&mut tcp, proxy.credentials.as_ref()).await.map(|()| tcp),
        _ => Err(proto_err("socks5: no acceptable auth method")),
    }
}

/// Sends a request (`cmd` + address) and returns the reply's bound address.
async fn command(tcp: &mut TcpStream, cmd: u8, addr: SocketAddr) -> io::Result<SocketAddr> {
    let mut req = vec![5, cmd, 0];
    put_addr(&mut req, addr);
    tcp.write_all(&req).await?;
    let mut head = [0u8; 3];
    tcp.read_exact(&mut head).await?;
    if head[1] != 0 {
        return Err(Error::other(format!("socks5: command {cmd} refused (code {})", head[1])));
    }
    read_addr(tcp).await
}

/// A TCP stream to `target` tunnelled through the proxy (CONNECT).
pub(crate) async fn connect_tcp(proxy: &Socks5Proxy, target: SocketAddr) -> io::Result<TcpStream> {
    let mut tcp = handshake(proxy).await?;
    command(&mut tcp, 1, target).await?;
    Ok(tcp)
}

impl Socks5Udp {
    pub async fn associate(proxy: &Socks5Proxy, target: SocketAddr) -> io::Result<Self> {
        let mut tcp = handshake(proxy).await?;
        let proxy_ip = tcp.peer_addr()?.ip();
        let bind: SocketAddr = if proxy_ip.is_ipv4() { (Ipv4Addr::UNSPECIFIED, 0).into() } else { (Ipv6Addr::UNSPECIFIED, 0).into() };
        let socket = UdpSocket::bind(bind).await?;
        let mut relay = command(&mut tcp, 3, bind).await?;
        if relay.ip().is_unspecified() {
            relay.set_ip(proxy_ip);
        }
        socket.connect(relay).await?;
        Ok(Self { _control: tcp, socket, target })
    }

    pub async fn send(&self, data: &[u8]) -> io::Result<()> {
        self.send_to(data, self.target).await
    }

    pub async fn send_to(&self, data: &[u8], target: SocketAddr) -> io::Result<()> {
        let mut buf = Vec::with_capacity(22 + data.len());
        buf.extend_from_slice(&[0, 0, 0]);
        put_addr(&mut buf, target);
        buf.extend_from_slice(data);
        self.socket.send(&buf).await.map(drop)
    }

    /// Receives one datagram into `buf`, returning the payload's byte range and its source.
    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(std::ops::Range<usize>, SocketAddr)> {
        loop {
            let n = self.socket.recv(buf).await?;
            if let Some((start, source)) = parse_udp_header(&buf[..n]) {
                return Ok((start..n, source.unwrap_or(self.target)));
            }
        }
    }
}

async fn authenticate(tcp: &mut TcpStream, creds: Option<&(String, String)>) -> io::Result<()> {
    let (user, pass) = creds.ok_or_else(|| proto_err("socks5: proxy requires credentials"))?;
    let (u, p) = (user.as_bytes(), pass.as_bytes());
    if u.len() > 255 || p.len() > 255 {
        return Err(proto_err("socks5: credentials longer than 255 bytes"));
    }
    let mut msg = vec![1, u.len() as u8];
    msg.extend_from_slice(u);
    msg.push(p.len() as u8);
    msg.extend_from_slice(p);
    tcp.write_all(&msg).await?;
    let mut status = [0u8; 2];
    tcp.read_exact(&mut status).await?;
    if status[1] != 0 {
        return Err(Error::new(ErrorKind::PermissionDenied, "socks5: authentication rejected"));
    }
    Ok(())
}

fn put_addr(out: &mut Vec<u8>, addr: SocketAddr) {
    match addr.ip() {
        IpAddr::V4(ip) => {
            out.push(1);
            out.extend_from_slice(&ip.octets());
        }
        IpAddr::V6(ip) => {
            out.push(4);
            out.extend_from_slice(&ip.octets());
        }
    }
    out.extend_from_slice(&addr.port().to_be_bytes());
}

async fn read_addr(tcp: &mut TcpStream) -> io::Result<SocketAddr> {
    let ip: IpAddr = match tcp.read_u8().await? {
        1 => {
            let mut b = [0u8; 4];
            tcp.read_exact(&mut b).await?;
            b.into()
        }
        4 => {
            let mut b = [0u8; 16];
            tcp.read_exact(&mut b).await?;
            b.into()
        }
        _ => return Err(proto_err("socks5: relay address must be an IP")),
    };
    Ok(SocketAddr::new(ip, tcp.read_u16().await?))
}

/// Length of the SOCKS5 UDP header and its source address (None for a domain name), or None for
/// fragmented/malformed datagrams (which we drop).
fn parse_udp_header(d: &[u8]) -> Option<(usize, Option<SocketAddr>)> {
    if d.len() < 4 || d[2] != 0 {
        return None;
    }
    let (len, ip): (usize, Option<IpAddr>) = match d[3] {
        1 => (4 + 4 + 2, Some(<[u8; 4]>::try_from(d.get(4..8)?).ok()?.into())),
        4 => (4 + 16 + 2, Some(<[u8; 16]>::try_from(d.get(4..20)?).ok()?.into())),
        3 => (4 + 1 + usize::from(*d.get(4)?) + 2, None),
        _ => return None,
    };
    let port = u16::from_be_bytes(d.get(len - 2..len)?.try_into().ok()?);
    Some((len, ip.map(|ip| SocketAddr::new(ip, port))))
}
