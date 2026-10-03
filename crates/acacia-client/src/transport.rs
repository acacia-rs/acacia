use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::ops::Range;

use rand_core::{OsRng, RngCore};
use tokio::net::UdpSocket;

use crate::socks5::{Socks5Proxy, Socks5Udp};
use crate::ConnectError;

/// `host[:port]` with the default Bedrock port filled in.
pub(crate) fn with_port(server: &str) -> String {
    let has_port = server.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok());
    if has_port { server.to_owned() } else { format!("{server}:19132") }
}

/// Resolves `host[:port]` (default port 19132), preferring IPv4.
pub(crate) async fn resolve(server: &str) -> Result<SocketAddr, ConnectError> {
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host(with_port(server)).await?.collect();
    addrs.iter().find(|a| a.is_ipv4()).or(addrs.first()).copied().ok_or(ConnectError::Resolve)
}

/// A RakNet client GUID as vanilla sends it: negative as an i64. go-raknet (Dragonfly) rejects a
/// positive one in OpenConnectionRequest2 and then blocks the address for 10 s.
pub(crate) fn raknet_client_guid() -> u64 {
    OsRng.next_u64() | 1 << 63
}

fn is_lan(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local(),
    }
}

/// A random address on the most common home subnets (192.168.0.0/24 and 192.168.1.0/24).
fn home_lan_address(port: u16) -> SocketAddr {
    let r = OsRng.next_u32();
    SocketAddr::new(Ipv4Addr::new(192, 168, (r & 1) as u8, 2 + ((r >> 8) % 252) as u8).into(), port)
}

/// UDP to the server, direct or relayed through a SOCKS5 proxy. A RakNet flow is connected to one
/// target; a NetherNet flow is unconnected because ICE may pick any of the server's candidates.
pub(crate) enum Transport {
    Direct { socket: UdpSocket, target: SocketAddr },
    Socks5(Socks5Udp),
}

impl Transport {
    /// A flow for RakNet: everything goes to `target`.
    pub async fn connect(target: SocketAddr, proxy: Option<&Socks5Proxy>) -> io::Result<Self> {
        let transport = Self::bind(target, proxy).await?;
        if let Self::Direct { socket, target } = &transport {
            socket.connect(*target).await?;
        }
        Ok(transport)
    }

    /// A flow for NetherNet: datagrams may go to any address via `send_to`.
    pub async fn bind(target: SocketAddr, proxy: Option<&Socks5Proxy>) -> io::Result<Self> {
        if let Some(proxy) = proxy {
            return Ok(Self::Socks5(Socks5Udp::associate(proxy, target).await?));
        }
        let bind: SocketAddr = if target.is_ipv4() { (Ipv4Addr::UNSPECIFIED, 0).into() } else { (Ipv6Addr::UNSPECIFIED, 0).into() };
        Ok(Self::Direct { socket: UdpSocket::bind(bind).await?, target })
    }

    /// The host candidate to advertise. Real players list only LAN addresses (the server finds their
    /// public one from their checks), so a public or proxy address is replaced by a plausible home
    /// LAN address; ICE still succeeds peer-reflexively.
    pub fn local_candidate(&self) -> io::Result<SocketAddr> {
        match self {
            Self::Direct { socket, target } => {
                // Connecting a UDP socket sends nothing; it only asks the OS for a route.
                let probe = std::net::UdpSocket::bind(SocketAddr::new(socket.local_addr()?.ip(), 0))?;
                probe.connect(target)?;
                let route = SocketAddr::new(probe.local_addr()?.ip(), socket.local_addr()?.port());
                Ok(if is_lan(route.ip()) { route } else { home_lan_address(route.port()) })
            }
            Self::Socks5(_) => Ok(home_lan_address(49152 + (OsRng.next_u32() % 16384) as u16)),
        }
    }

    pub async fn send(&self, data: &[u8]) -> io::Result<()> {
        match self {
            Self::Direct { socket, .. } => socket.send(data).await.map(drop),
            Self::Socks5(s) => s.send(data).await,
        }
    }

    pub async fn send_to(&self, data: &[u8], target: SocketAddr) -> io::Result<()> {
        match self {
            Self::Direct { socket, .. } => socket.send_to(data, target).await.map(drop),
            Self::Socks5(s) => s.send_to(data, target).await,
        }
    }

    /// Receives one datagram into `buf`, returning the payload's byte range and its source.
    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(Range<usize>, SocketAddr)> {
        match self {
            Self::Direct { socket, .. } => loop {
                match socket.recv_from(buf).await {
                    Ok((n, source)) => return Ok((0..n, source)),
                    // Windows surfaces ICMP port-unreachable as a reset on UDP sockets.
                    Err(e) if e.kind() == io::ErrorKind::ConnectionReset => continue,
                    Err(e) => return Err(e),
                }
            },
            Self::Socks5(s) => s.recv_from(buf).await,
        }
    }
}
