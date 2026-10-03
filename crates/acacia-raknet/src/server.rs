use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::{Bytes, BytesMut};

use crate::conn::{Conn, ConnConfig};
use crate::reliability::RecvLimits;
use crate::types::DisconnectReason;
use crate::wire::datagram::{Reliability, FLAG_VALID};
use crate::wire::{connected as c, offline as o, WireError};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub guid: u64,
    /// Answer to unconnected pings (`MCPE;...`); see [`Server::set_motd`].
    pub motd: String,
    pub protocol_version: u8,
    pub max_mtu: u16,
    pub idle_timeout: Duration,
    pub ping_interval: Duration,
    pub recv_limits: RecvLimits,
}

impl ServerConfig {
    pub fn new(guid: u64, motd: String) -> Self {
        Self {
            guid,
            motd,
            protocol_version: 11,
            max_mtu: 1492,
            idle_timeout: Duration::from_secs(10),
            ping_interval: Duration::from_secs(2),
            recv_limits: RecvLimits::SERVER,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerEvent {
    Connected(SocketAddr),
    /// A message from a connected peer (the first byte is its ID; Bedrock game batches start with 0xfe).
    Message(SocketAddr, Bytes),
    Disconnected(SocketAddr, DisconnectReason),
}

struct Peer {
    conn: Conn,
    connected: bool,
}

/// A network-free RakNet server: answers pings and the offline handshake, then keeps one connection
/// per peer address. Feed it datagrams and timeouts, drain `(address, datagram)` pairs and events.
pub struct Server {
    cfg: ServerConfig,
    epoch: Instant,
    peers: HashMap<SocketAddr, Peer>,
    outbox: VecDeque<(SocketAddr, Bytes)>,
    events: VecDeque<ServerEvent>,
}

impl Server {
    pub fn new(mut cfg: ServerConfig, now: Instant) -> Self {
        cfg.max_mtu = cfg.max_mtu.max(o::MIN_MTU);
        Self { cfg, epoch: now, peers: HashMap::new(), outbox: VecDeque::new(), events: VecDeque::new() }
    }

    pub fn set_motd(&mut self, motd: String) {
        self.cfg.motd = motd;
    }

    /// Queues a message on ordering channel 0. Returns false if `peer` is not connected.
    pub fn send(&mut self, peer: SocketAddr, data: Bytes, reliability: Reliability) -> bool {
        match self.peers.get_mut(&peer) {
            Some(p) if p.connected => {
                p.conn.queue(data, reliability);
                true
            }
            _ => false,
        }
    }

    /// Sends `peer` a disconnection notification and forgets it; no event follows.
    pub fn close(&mut self, peer: SocketAddr, now: Instant) {
        if let Some(mut p) = self.peers.remove(&peer) {
            self.outbox.extend(p.conn.close(now).into_iter().map(|d| (peer, d)));
        }
    }

    pub fn poll_event(&mut self) -> Option<ServerEvent> {
        self.events.pop_front()
    }

    pub fn poll_transmit(&mut self, now: Instant) -> Option<(SocketAddr, Bytes)> {
        if let Some(out) = self.outbox.pop_front() {
            return Some(out);
        }
        self.peers.iter_mut().find_map(|(&addr, p)| p.conn.poll_transmit(now).map(|d| (addr, d)))
    }

    pub fn poll_timeout(&self) -> Option<Instant> {
        self.peers.values().map(|p| p.conn.poll_timeout()).min()
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        let mut lost = Vec::new();
        for (&addr, p) in &mut self.peers {
            if let Err(reason) = p.conn.handle_timeout(now) {
                lost.push((addr, reason));
            }
        }
        for (addr, reason) in lost {
            self.drop_peer(addr, reason);
        }
    }

    pub fn handle_datagram(&mut self, now: Instant, from: SocketAddr, data: Bytes) {
        let Some(&id) = data.first() else { return };
        let result = match self.peers.get_mut(&from) {
            Some(p) if id & FLAG_VALID != 0 => p.conn.handle_datagram(now, &data).and_then(|()| self.handle_messages(now, from)),
            _ => self.handle_offline(now, from, id, &data).map_err(|e| DisconnectReason::Protocol(e.to_string())),
        };
        if let Err(reason) = result {
            self.drop_peer(from, reason);
        }
    }

    fn drop_peer(&mut self, addr: SocketAddr, reason: DisconnectReason) {
        if let Some(p) = self.peers.remove(&addr)
            && p.connected
        {
            self.events.push_back(ServerEvent::Disconnected(addr, reason));
        }
    }

    fn handle_offline(&mut self, now: Instant, from: SocketAddr, id: u8, data: &[u8]) -> Result<(), WireError> {
        let mut out = BytesMut::new();
        match id {
            _ if o::is_unconnected_ping(id) => {
                let time = o::parse_unconnected_ping(data)?;
                o::unconnected_pong(&mut out, time, self.cfg.guid, &self.cfg.motd);
            }
            o::ID_OPEN_CONNECTION_REQUEST_1 => {
                let (protocol, mtu) = o::parse_request_1(data)?;
                if mtu < o::MIN_MTU {
                    return Ok(());
                }
                if protocol == self.cfg.protocol_version {
                    o::reply_1(&mut out, self.cfg.guid, mtu.min(self.cfg.max_mtu));
                } else {
                    o::incompatible_protocol(&mut out, self.cfg.protocol_version, self.cfg.guid);
                }
            }
            o::ID_OPEN_CONNECTION_REQUEST_2 => {
                let (mtu, _guid) = o::parse_request_2(data)?;
                if mtu < o::MIN_MTU {
                    return Ok(());
                }
                let mtu = mtu.min(self.cfg.max_mtu);
                // A repeated request 2 (our reply got lost) or a rejoin from the same port starts over.
                self.drop_peer(from, DisconnectReason::ClientClosed);
                let cfg = ConnConfig {
                    mtu,
                    idle_timeout: self.cfg.idle_timeout,
                    ping_interval: self.cfg.ping_interval,
                    recv_limits: self.cfg.recv_limits,
                };
                let conn = Conn::new(self.epoch, cfg, now);
                self.peers.insert(from, Peer { conn, connected: false });
                o::reply_2(&mut out, self.cfg.guid, from, mtu);
            }
            _ => return Ok(()),
        }
        self.outbox.push_back((from, out.freeze()));
        Ok(())
    }

    fn handle_messages(&mut self, now: Instant, from: SocketAddr) -> Result<(), DisconnectReason> {
        let protocol = |e: WireError| DisconnectReason::Protocol(e.to_string());
        while let Some(p) = self.peers.get_mut(&from)
            && let Some(msg) = p.conn.pop_message(now)
        {
            match msg[0] {
                c::ID_CONNECTION_REQUEST if !p.connected => {
                    let request_time = c::parse_connection_request(&msg).map_err(protocol)?;
                    let accepted = c::connection_request_accepted(from, request_time, p.conn.time(now));
                    p.conn.queue(accepted, Reliability::Reliable);
                }
                c::ID_NEW_INCOMING_CONNECTION if !p.connected => {
                    p.connected = true;
                    self.events.push_back(ServerEvent::Connected(from));
                }
                c::ID_DISCONNECTION_NOTIFICATION => return Err(DisconnectReason::ClientClosed),
                _ if p.connected => self.events.push_back(ServerEvent::Message(from, msg)),
                _ => {}
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Client, Config, Event};

    #[test]
    fn requests_below_the_mtu_floor_are_ignored() {
        let now = Instant::now();
        let (server_addr, from): (SocketAddr, SocketAddr) = ("127.0.0.1:19132".parse().unwrap(), "127.0.0.1:50000".parse().unwrap());
        let mut server = Server::new(ServerConfig::new(1, "MCPE;test".into()), now);
        let mut request_1 = BytesMut::new();
        o::open_connection_request_1(&mut request_1, 11, 100);
        server.handle_datagram(now, from, request_1.freeze());
        for mtu in [0, 20, 44, o::MIN_MTU - 1] {
            let mut request_2 = BytesMut::new();
            o::open_connection_request_2(&mut request_2, None, server_addr, mtu, 2);
            server.handle_datagram(now, from, request_2.freeze());
        }
        assert_eq!(server.poll_transmit(now), None);
        assert!(server.peers.is_empty());
    }

    #[test]
    fn client_survives_a_tiny_mtu_in_the_replies() {
        let now = Instant::now();
        let server_addr: SocketAddr = "127.0.0.1:19132".parse().unwrap();
        let mut client = Client::new(Config::new(2), server_addr, now);
        let (mut reply_1, mut reply_2) = (BytesMut::new(), BytesMut::new());
        o::reply_1(&mut reply_1, 1, 0);
        o::reply_2(&mut reply_2, 1, "127.0.0.1:50000".parse().unwrap(), 0);
        client.handle_datagram(now, reply_1.freeze());
        client.handle_datagram(now, reply_2.freeze());
        let sent: Vec<Bytes> = std::iter::from_fn(|| client.poll_transmit(now)).collect();
        assert!(sent.len() >= 3, "requests 1 and 2, then the connection request");
    }

    #[test]
    fn client_connects_and_messages_flow_both_ways() {
        let now = Instant::now();
        let (server_addr, client_addr): (SocketAddr, SocketAddr) = ("127.0.0.1:19132".parse().unwrap(), "127.0.0.1:50000".parse().unwrap());
        let mut server = Server::new(ServerConfig::new(1, "MCPE;test".into()), now);
        let mut client = Client::new(Config::new(2), server_addr, now);
        let big = Bytes::from(vec![7u8; 5000]);
        let hello = Bytes::from_static(b"\xfehello");
        let (mut to_client, mut to_server) = (Vec::new(), Vec::new());
        for _ in 0..10 {
            while let Some(d) = client.poll_transmit(now) {
                server.handle_datagram(now, client_addr, d);
            }
            while let Some((to, d)) = server.poll_transmit(now) {
                assert_eq!(to, client_addr);
                client.handle_datagram(now, d);
            }
            while let Some(e) = client.poll_event() {
                match e {
                    Event::Connected { .. } => assert!(client.send(big.clone(), Reliability::ReliableOrdered)),
                    Event::Message(m) => to_client.push(m),
                    Event::Disconnected(r) => panic!("client lost: {r:?}"),
                }
            }
            while let Some(e) = server.poll_event() {
                match e {
                    ServerEvent::Connected(peer) => assert!(server.send(peer, hello.clone(), Reliability::ReliableOrdered)),
                    ServerEvent::Message(_, m) => to_server.push(m),
                    ServerEvent::Disconnected(_, r) => panic!("server lost the client: {r:?}"),
                }
            }
        }
        assert_eq!((to_server, to_client), (vec![big], vec![hello]));

        client.close(now);
        while let Some(d) = client.poll_transmit(now) {
            server.handle_datagram(now, client_addr, d);
        }
        assert_eq!(server.poll_event(), Some(ServerEvent::Disconnected(client_addr, DisconnectReason::ClientClosed)));
    }
}
