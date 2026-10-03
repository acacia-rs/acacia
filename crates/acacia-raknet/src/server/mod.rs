mod config;
mod cookie;
mod handshake;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, SocketAddr};
use std::time::Instant;

use bytes::Bytes;

pub use config::{ServerConfig, ServerEvent};
use cookie::Cookies;

use crate::conn::Conn;
use crate::types::DisconnectReason;
use crate::wire::datagram::{Reliability, FLAG_VALID};
use crate::wire::{connected as c, offline as o, WireError};

struct Peer {
    conn: Conn,
    guid: u64,
    connected: bool,
    /// Only meaningful until `connected`.
    handshake_deadline: Instant,
}

impl Peer {
    fn poll_timeout(&self) -> Instant {
        let conn = self.conn.poll_timeout();
        if self.connected { conn } else { conn.min(self.handshake_deadline) }
    }

    fn handle_timeout(&mut self, now: Instant) -> Result<(), DisconnectReason> {
        if !self.connected && now >= self.handshake_deadline {
            return Err(DisconnectReason::ConnectTimeout);
        }
        self.conn.handle_timeout(now)
    }
}

/// A network-free RakNet server: answers pings and the offline handshake, then keeps one connection
/// per peer address. Feed it datagrams and timeouts, drain `(address, datagram)` pairs and events.
pub struct Server {
    cfg: ServerConfig,
    epoch: Instant,
    cookies: Cookies,
    peers: HashMap<SocketAddr, Peer>,
    banned: HashSet<IpAddr>,
    outbox: VecDeque<(SocketAddr, Bytes)>,
    events: VecDeque<ServerEvent>,
}

impl Server {
    pub fn new(mut cfg: ServerConfig, now: Instant) -> Self {
        cfg.max_mtu = cfg.max_mtu.max(o::MIN_MTU);
        let cookies = Cookies::new(cfg.cookie_secret);
        Self { cfg, epoch: now, cookies, peers: HashMap::new(), banned: HashSet::new(), outbox: VecDeque::new(), events: VecDeque::new() }
    }

    pub fn set_motd(&mut self, motd: String) {
        self.cfg.motd = motd;
    }

    /// Refuses new connections from `ip` with "connection banned" until [`Server::unban`].
    /// Peers already connected from it stay; [`Server::close`] them. Expiry is the caller's policy.
    pub fn ban(&mut self, ip: IpAddr) {
        self.banned.insert(ip);
    }

    pub fn unban(&mut self, ip: IpAddr) {
        self.banned.remove(&ip);
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
        self.peers.values().map(Peer::poll_timeout).min()
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        let mut lost = Vec::new();
        for (&addr, p) in &mut self.peers {
            if let Err(reason) = p.handle_timeout(now) {
                lost.push((addr, reason));
            }
        }
        for (addr, reason) in lost {
            self.drop_peer(addr, reason);
        }
    }

    pub fn handle_datagram(&mut self, now: Instant, from: SocketAddr, data: Bytes) {
        let Some(&id) = data.first() else { return };
        match self.peers.get_mut(&from) {
            Some(p) if id & FLAG_VALID != 0 => {
                if let Err(reason) = p.conn.handle_datagram(now, &data).and_then(|()| self.handle_messages(now, from)) {
                    self.drop_peer(from, reason);
                }
            }
            // Offline datagrams prove nothing about their sender, so a bad one never costs a peer.
            _ => _ = self.handle_offline(now, from, id, &data),
        }
    }

    fn drop_peer(&mut self, addr: SocketAddr, reason: DisconnectReason) {
        if let Some(p) = self.peers.remove(&addr)
            && p.connected
        {
            self.events.push_back(ServerEvent::Disconnected(addr, reason));
        }
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
