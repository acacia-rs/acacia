mod flow;
mod handshake;

use std::time::Duration;

use bytes::BytesMut;

use super::*;
use crate::{Client, Config, Event};

const TICK: Duration = Duration::from_millis(10);
const SERVER_PORT: u16 = 19132;

fn addr(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn config() -> ServerConfig {
    ServerConfig::new(1, "MCPE;test".into())
}

fn request_1(mtu: u16) -> BytesMut {
    let mut out = BytesMut::new();
    o::open_connection_request_1(&mut out, 11, mtu);
    out
}

fn request_2(cookie: Option<u32>, mtu: u16, guid: u64) -> BytesMut {
    let mut out = BytesMut::new();
    o::open_connection_request_2(&mut out, cookie, addr(SERVER_PORT), mtu, guid);
    out
}

/// A server and its clients on a lossless in-memory network with a virtual clock.
struct Net {
    now: Instant,
    server: Server,
    clients: Vec<(SocketAddr, Client)>,
    /// Every server event so far.
    events: Vec<ServerEvent>,
}

impl Net {
    fn new(cfg: ServerConfig) -> Self {
        let now = Instant::now();
        Self { now, server: Server::new(cfg, now), clients: Vec::new(), events: Vec::new() }
    }

    /// Starts a client on `port` (also its GUID) and returns its index.
    fn join(&mut self, port: u16) -> usize {
        self.clients.push((addr(port), Client::new(Config::new(port.into()), addr(SERVER_PORT), self.now)));
        self.clients.len() - 1
    }

    /// Carries datagrams both ways, one exchange per tick of the clock.
    fn run(&mut self, ticks: usize) {
        for _ in 0..ticks {
            for (from, client) in &mut self.clients {
                while let Some(d) = client.poll_transmit(self.now) {
                    self.server.handle_datagram(self.now, *from, d);
                }
            }
            while let Some((to, d)) = self.server.poll_transmit(self.now) {
                if let Some((_, client)) = self.clients.iter_mut().find(|(a, _)| *a == to) {
                    client.handle_datagram(self.now, d);
                }
            }
            self.events.extend(std::iter::from_fn(|| self.server.poll_event()));
            self.now += TICK;
            self.server.handle_timeout(self.now);
            for (_, client) in &mut self.clients {
                client.handle_timeout(self.now);
            }
        }
    }

    fn client_events(&mut self, client: usize) -> Vec<Event> {
        std::iter::from_fn(|| self.clients[client].1.poll_event()).collect()
    }

    /// The cookie reply 1 would hand `from` right now.
    fn cookie(&self, from: SocketAddr) -> u32 {
        self.server.cookies.issue(from, self.now - self.server.epoch)
    }

    /// What the server answers `from` for one raw datagram. Call it while nothing else is in transit.
    fn raw(&mut self, from: SocketAddr, datagram: BytesMut) -> Vec<Bytes> {
        self.server.handle_datagram(self.now, from, datagram.freeze());
        std::iter::from_fn(|| self.server.poll_transmit(self.now)).filter(|(to, _)| *to == from).map(|(_, d)| d).collect()
    }
}
