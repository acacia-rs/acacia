//! Vanilla's LAN discovery socket and broadcast addresses: docs/research/qos-lan-capture-2026-10-03.md "LAN rules from BDS".

use std::collections::BTreeSet;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use acacia_nethernet::lan::DISCOVERY_PORT;
use if_addrs::IfAddr;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;

/// Where requests go until the interfaces are known.
pub(super) const FALLBACK: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), DISCOVERY_PORT);
const ALL_NODES: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 1);

/// A dual-stack socket on 7551 (ephemeral if taken), IPv4-only when IPv6 is unavailable.
pub(super) struct LanSocket {
    socket: UdpSocket,
    v6: bool,
}

impl LanSocket {
    pub(super) fn bind() -> io::Result<Self> {
        match bind(Domain::IPV6) {
            Ok(socket) => Ok(Self { socket, v6: true }),
            Err(e) => {
                tracing::debug!("no IPv6 LAN socket, using IPv4: {e}");
                Ok(Self { socket: bind(Domain::IPV4)?, v6: false })
            }
        }
    }

    pub(super) async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        let addr = match addr {
            SocketAddr::V4(v4) if self.v6 => SocketAddr::new(IpAddr::V6(v4.ip().to_ipv6_mapped()), v4.port()),
            addr => addr,
        };
        self.socket.send_to(buf, addr).await
    }

    /// The sender comes back as plain IPv4 when it was IPv4-mapped.
    pub(super) async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let (n, from) = self.socket.recv_from(buf).await?;
        Ok((n, SocketAddr::new(from.ip().to_canonical(), from.port())))
    }

    /// Every interface's broadcast address: IPv4 subnet broadcasts (not loopback or 169.254/16), and
    /// `ff02::1` once when any interface has IPv6 and the socket can reach it.
    pub(super) fn broadcast_domain(&self) -> io::Result<Vec<SocketAddr>> {
        let mut domain = BTreeSet::new();
        for iface in if_addrs::get_if_addrs()? {
            match iface.addr {
                IfAddr::V4(a) if !a.ip.is_loopback() && !a.ip.is_link_local() => {
                    domain.insert(IpAddr::V4(subnet_broadcast(a.ip, a.prefixlen)));
                }
                IfAddr::V6(a) if self.v6 && !a.ip.is_loopback() => {
                    domain.insert(IpAddr::V6(ALL_NODES));
                }
                _ => {}
            }
        }
        Ok(domain.into_iter().map(|ip| SocketAddr::new(ip, DISCOVERY_PORT)).collect())
    }
}

fn bind(domain: Domain) -> io::Result<UdpSocket> {
    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
    if domain == Domain::IPV6 {
        socket.set_only_v6(false)?;
    }
    socket.set_broadcast(true)?;
    socket.set_nonblocking(true)?;
    let any: IpAddr = if domain == Domain::IPV6 { Ipv6Addr::UNSPECIFIED.into() } else { Ipv4Addr::UNSPECIFIED.into() };
    if let Err(e) = socket.bind(&SocketAddr::new(any, DISCOVERY_PORT).into()) {
        if e.kind() != io::ErrorKind::AddrInUse {
            return Err(e);
        }
        tracing::debug!("LAN port {DISCOVERY_PORT} already occupied, using an ephemeral port");
        socket.bind(&SocketAddr::new(any, 0).into())?;
    }
    UdpSocket::from_std(socket.into())
}

fn subnet_broadcast(ip: Ipv4Addr, prefix: u8) -> Ipv4Addr {
    let host_bits = u32::MAX.checked_shr(u32::from(prefix)).unwrap_or(0);
    Ipv4Addr::from(u32::from(ip) | host_bits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subnet_broadcast_sets_host_bits() {
        assert_eq!(subnet_broadcast(Ipv4Addr::new(192, 168, 1, 236), 24), Ipv4Addr::new(192, 168, 1, 255));
        assert_eq!(subnet_broadcast(Ipv4Addr::new(10, 1, 2, 3), 8), Ipv4Addr::new(10, 255, 255, 255));
        assert_eq!(subnet_broadcast(Ipv4Addr::new(10, 1, 2, 3), 32), Ipv4Addr::new(10, 1, 2, 3));
        assert_eq!(subnet_broadcast(Ipv4Addr::new(10, 1, 2, 3), 0), Ipv4Addr::BROADCAST);
    }
}
