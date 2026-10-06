//! Following transfers over RakNet. A server's Transfer names another server; the relay points it
//! at the proxy instead (relay.rs), at a port opened for that one game: a door. Whoever joins
//! through a door goes where its game was sent, so games are told apart by the port they come
//! back on, and a new join on the proxy's own port is never mistaken for one.

use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// How long a transferred game has to come back.
const WINDOW: Duration = Duration::from_secs(30);

/// A datagram that arrived at a door: its port, the sender and the bytes.
pub type Arrival = (u16, SocketAddr, Bytes);

struct Door {
    socket: Arc<UdpSocket>,
    reader: JoinHandle<()>,
    /// The connected game this door was opened for, until it leaves.
    holder: Option<SocketAddr>,
    /// Where the holder was transferred to, and when.
    target: Option<((String, u16), Instant)>,
}

/// The doors of every connected game, by port.
pub struct Doors {
    doors: HashMap<u16, Door>,
    arrivals: mpsc::UnboundedSender<Arrival>,
}

impl Doors {
    /// What the doors receive comes out of `arrivals`.
    pub fn new(arrivals: mpsc::UnboundedSender<Arrival>) -> Doors {
        Doors { doors: HashMap::new(), arrivals }
    }

    /// Opens a door on `ip` for `game` to be transferred through; returns its port.
    pub async fn open(&mut self, ip: IpAddr, game: SocketAddr) -> io::Result<u16> {
        let socket = Arc::new(UdpSocket::bind((ip, 0)).await?);
        let port = socket.local_addr()?.port();
        let reader = tokio::spawn(read(socket.clone(), port, self.arrivals.clone()));
        self.doors.insert(port, Door { socket, reader, holder: Some(game), target: None });
        Ok(port)
    }

    /// `game`'s server transferred it: its door now leads to `target`.
    pub fn lead(&mut self, game: SocketAddr, target: (String, u16), now: Instant) {
        if let Some(door) = self.doors.values_mut().find(|door| door.holder == Some(game)) {
            door.target = Some((target, now));
        }
    }

    /// Where a game joining through `port` is headed. It may try more than once inside the window.
    pub fn target(&self, port: u16, now: Instant) -> Option<(String, u16)> {
        let (target, set) = self.doors.get(&port)?.target.as_ref()?;
        (now.duration_since(*set) <= WINDOW).then(|| target.clone())
    }

    pub fn socket(&self, port: u16) -> Option<&Arc<UdpSocket>> {
        self.doors.get(&port).map(|door| &door.socket)
    }

    /// `game` left: its door stays only if it leads somewhere.
    pub fn left(&mut self, game: SocketAddr) {
        for door in self.doors.values_mut().filter(|door| door.holder == Some(game)) {
            door.holder = None;
        }
    }

    /// Closes the doors nobody needs: no holder, no target inside the window, and no game that
    /// came in through them (`in_use`) still connected.
    pub fn sweep(&mut self, now: Instant, in_use: impl Fn(u16) -> bool) {
        let live: Vec<u16> = self.doors.keys().copied().filter(|&port| self.target(port, now).is_some()).collect();
        self.doors.retain(|port, door| {
            let keep = door.holder.is_some() || live.contains(port) || in_use(*port);
            if !keep {
                door.reader.abort();
            }
            keep
        });
    }

    #[cfg(test)]
    fn ports(&self) -> Vec<u16> {
        self.doors.keys().copied().collect()
    }
}

async fn read(socket: Arc<UdpSocket>, port: u16, tx: mpsc::UnboundedSender<Arrival>) {
    let mut buf = vec![0u8; 2048];
    loop {
        match socket.recv_from(&mut buf).await {
            Ok((n, from)) => {
                if tx.send((port, from, Bytes::copy_from_slice(&buf[..n]))).is_err() {
                    return;
                }
            }
            // Windows reports ICMP port-unreachable from an earlier send_to as an error here.
            Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {}
            Err(_) => return,
        }
    }
}

/// The proxy's address as `game` reaches it: the one the OS routes to the game from, on `port`.
pub fn proxy_addr(game: SocketAddr, port: u16) -> io::Result<SocketAddr> {
    let route = std::net::UdpSocket::bind(any_port(game))?;
    route.connect(game)?;
    Ok(SocketAddr::new(route.local_addr()?.ip(), port))
}

/// The address to bind a socket that will talk to `peer`: any port, in its address family.
pub fn any_port(peer: SocketAddr) -> SocketAddr {
    let any: IpAddr = if peer.is_ipv4() { Ipv4Addr::UNSPECIFIED.into() } else { Ipv6Addr::UNSPECIFIED.into() };
    SocketAddr::new(any, 0)
}

/// A transfer target's address; IPv4 when the name has both.
pub async fn resolve(host: &str, port: u16) -> io::Result<SocketAddr> {
    let mut found: Vec<SocketAddr> = tokio::net::lookup_host((host, port)).await?.collect();
    found.sort_by_key(|addr| !addr.is_ipv4());
    found.first().copied().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("{host}: no address")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(port: u16) -> SocketAddr {
        SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port)
    }

    #[tokio::test]
    async fn a_door_leads_only_its_own_game_and_only_inside_the_window() {
        let (tx, _arrivals) = mpsc::unbounded_channel();
        let (mut doors, now) = (Doors::new(tx), Instant::now());
        let (first, second) = (addr(1001), addr(1002));
        let a = doors.open(Ipv4Addr::LOCALHOST.into(), first).await.unwrap();
        let b = doors.open(Ipv4Addr::LOCALHOST.into(), second).await.unwrap();
        assert_eq!((doors.target(a, now), doors.target(b, now)), (None, None), "no door leads anywhere before a transfer");

        // Two games behind one address transfer at once: each door keeps its own target.
        doors.lead(first, ("lobby.example".into(), 19132), now);
        doors.lead(second, ("game.example".into(), 19133), now);
        assert_eq!(doors.target(a, now + WINDOW), Some(("lobby.example".into(), 19132)));
        assert_eq!(doors.target(a, now + WINDOW), Some(("lobby.example".into(), 19132)), "a second try still gets through");
        assert_eq!(doors.target(b, now), Some(("game.example".into(), 19133)));
        assert_eq!(doors.target(a, now + WINDOW + Duration::from_millis(1)), None);
    }

    #[tokio::test]
    async fn doors_close_when_nobody_needs_them() {
        let (tx, _arrivals) = mpsc::unbounded_channel();
        let (mut doors, now) = (Doors::new(tx), Instant::now());
        let (stays, leaves, transfers) = (addr(1001), addr(1002), addr(1003));
        let kept = doors.open(Ipv4Addr::LOCALHOST.into(), stays).await.unwrap();
        doors.open(Ipv4Addr::LOCALHOST.into(), leaves).await.unwrap();
        let led = doors.open(Ipv4Addr::LOCALHOST.into(), transfers).await.unwrap();
        doors.lead(transfers, ("game.example".into(), 19133), now);
        doors.left(leaves);
        doors.left(transfers);

        doors.sweep(now, |_| false);
        let mut open = doors.ports();
        open.sort_unstable();
        let mut expected = vec![kept, led];
        expected.sort_unstable();
        assert_eq!(open, expected, "a connected game's door and a door that leads somewhere stay");

        // Past the window the door stays for as long as the game that came through it is connected.
        let late = now + WINDOW + Duration::from_secs(1);
        doors.sweep(late, |port| port == led);
        assert!(doors.socket(led).is_some());
        doors.sweep(late, |_| false);
        assert_eq!(doors.ports(), [kept]);
    }
}
