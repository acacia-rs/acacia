//! Following transfers over RakNet. A server's Transfer names another server; the relay points it
//! at the proxy instead (relay.rs) and the real target waits here for that game's next connection.
//! Games are told apart by IP alone (the reconnect comes from a new port), so two games behind one
//! address that transfer within [`WINDOW`] of each other can swap targets.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::{Duration, Instant};

/// How long a transferred game has to come back.
const WINDOW: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct Routes(HashMap<IpAddr, (String, u16, Instant)>);

impl Routes {
    pub fn set(&mut self, game: IpAddr, (host, port): (String, u16), now: Instant) {
        self.0.insert(game, (host, port, now));
    }

    /// Where the game at this address was last transferred to, once.
    pub fn take(&mut self, game: IpAddr, now: Instant) -> Option<(String, u16)> {
        let (host, port, set) = self.0.remove(&game)?;
        (now.duration_since(set) <= WINDOW).then_some((host, port))
    }
}

/// The proxy's address as `game` reaches it: the one the OS routes to the game from, on `port`.
pub fn proxy_addr(game: SocketAddr, port: u16) -> io::Result<SocketAddr> {
    let any: IpAddr = if game.is_ipv4() { Ipv4Addr::UNSPECIFIED.into() } else { Ipv6Addr::UNSPECIFIED.into() };
    let route = std::net::UdpSocket::bind((any, 0))?;
    route.connect(game)?;
    Ok(SocketAddr::new(route.local_addr()?.ip(), port))
}

/// Upstream sockets are IPv4 (raknet.rs).
pub async fn resolve(host: &str, port: u16) -> io::Result<SocketAddr> {
    let found = tokio::net::lookup_host((host, port)).await?.find(SocketAddr::is_ipv4);
    found.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{host}: no IPv4 address")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_route_is_taken_once_and_only_inside_the_window() {
        let (mut routes, game, now) = (Routes::default(), IpAddr::from(Ipv4Addr::LOCALHOST), Instant::now());
        routes.set(game, ("lobby.example".into(), 19132), now);
        assert_eq!(routes.take(game, now + WINDOW), Some(("lobby.example".into(), 19132)));
        assert_eq!(routes.take(game, now + WINDOW), None);

        routes.set(game, ("lobby.example".into(), 19132), now);
        assert_eq!(routes.take(game, now + WINDOW + Duration::from_millis(1)), None);
    }
}
