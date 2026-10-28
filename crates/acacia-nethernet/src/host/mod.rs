//! Many answering connections behind one UDP socket: a network-free NetherNet host. Signaling
//! (HTTP, LAN) hands it offers; it routes each datagram to its connection, by the ICE username of
//! binding requests and by source address after that. A datagram it does not claim belongs to
//! whatever else shares the socket (RakNet). With [`IceConfig`] servers it also offers srflx and
//! relay candidates (gather.rs), whose STUN/TURN traffic shares the socket too.

mod answers;
mod gather;
#[cfg(test)]
mod relay_tests;
mod route;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::Bytes;
use p384::ecdsa::SigningKey;

pub use gather::{IceConfig, TurnServer};

use crate::{Connection, Event, LocalCandidate};
use gather::{Claim, Gather};

/// Paths one peer may hold; ICE settles on one, a second network path or a relay adds a few.
const MAX_ADDRS: usize = 8;
/// Addresses one peer may open on our TURN allocations.
const MAX_PERMITS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerId(pub u64);

#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Connections held at once, opening ones included.
    pub max_peers: usize,
    /// From the answer until both data channels are open.
    pub handshake_timeout: Duration,
    /// STUN/TURN servers; none by default, so answers carry the host candidate only.
    pub ice: IceConfig,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self { max_peers: 1024, handshake_timeout: Duration::from_secs(15), ice: IceConfig::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostEvent {
    /// Both data channels are open; messages follow.
    Open(PeerId),
    Message(PeerId, Bytes),
    /// Exactly one per answered offer, whether or not it opened.
    Closed(PeerId, String),
}

/// What to signal back for an offer.
#[derive(Debug, Clone)]
pub struct Answer {
    pub peer: PeerId,
    pub sdp: String,
    /// `candidate:` lines to send as `CANDIDATEADD`s; empty when the SDP embeds them.
    pub candidates: Vec<String>,
}

/// Where a datagram reached us: the shared socket (`None`) or one of our TURN relays.
type Path = (Option<SocketAddr>, SocketAddr);

struct Peer {
    conn: Connection,
    addrs: Vec<Path>,
    /// Remote addresses given TURN permissions for this peer.
    permits: Vec<SocketAddr>,
    /// Until the channels open.
    open_by: Option<Instant>,
}

pub struct Host {
    /// Signs every answer; clients pin it per server address.
    key: SigningKey,
    config: HostConfig,
    peers: HashMap<PeerId, Peer>,
    by_ufrag: HashMap<String, PeerId>,
    by_addr: HashMap<Path, PeerId>,
    next_id: u64,
    gather: Gather,
    transmits: VecDeque<(SocketAddr, Vec<u8>)>,
    events: VecDeque<HostEvent>,
}

impl Host {
    pub fn new(key: SigningKey, config: HostConfig) -> Self {
        Self {
            key,
            gather: Gather::new(config.ice.clone()),
            config,
            peers: HashMap::new(),
            by_ufrag: HashMap::new(),
            by_addr: HashMap::new(),
            next_id: 1,
            transmits: VecDeque::new(),
            events: VecDeque::new(),
        }
    }

    /// Starts asking the configured STUN and TURN servers for candidates; answers carry them once
    /// the servers reply. Without servers it does nothing.
    pub fn gather(&mut self, now: Instant) {
        self.gather.start(now);
        self.drain_gather();
    }

    /// The srflx and relay candidates gathered so far, for a host candidate at `host`.
    pub fn gathered(&self, host: SocketAddr) -> Vec<LocalCandidate> {
        self.gather.candidates(host)
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    /// Takes a datagram from the shared socket. False if it belongs to no connection here.
    pub fn handle_datagram(&mut self, now: Instant, source: SocketAddr, data: &[u8]) -> bool {
        let (id, via, from, data) = match self.gather.handle_datagram(now, source, data) {
            Some(Claim::Consumed) => {
                self.drain_gather();
                return true;
            }
            Some(Claim::Relayed { relay, peer, data }) => {
                self.drain_gather();
                (self.route(now, Some(relay), peer, &data), Some(relay), peer, std::borrow::Cow::Owned(data))
            }
            None => (self.route(now, None, source, data), None, source, std::borrow::Cow::Borrowed(data)),
        };
        let Some(id) = id else { return via.is_some() };
        if let Some(peer) = self.peers.get_mut(&id) {
            match via {
                Some(relay) => peer.conn.handle_relayed(now, relay, from, &data),
                None => peer.conn.handle_datagram(now, from, &data),
            }
        }
        self.pump(id, now);
        true
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        if self.gather.poll_timeout().is_some_and(|t| t <= now) {
            self.gather.handle_timeout(now);
            self.drain_gather();
        }
        let due: Vec<PeerId> = self
            .peers
            .iter()
            .filter(|(_, p)| p.conn.poll_timeout().into_iter().chain(p.open_by).any(|t| t <= now))
            .map(|(&id, _)| id)
            .collect();
        for id in due {
            let Some(peer) = self.peers.get_mut(&id) else { continue };
            if peer.open_by.is_some_and(|t| t <= now) {
                tracing::debug!(?id, "nethernet handshake timed out");
                peer.conn.close(now);
            } else {
                peer.conn.handle_timeout(now);
            }
            self.pump(id, now);
        }
    }

    pub fn poll_timeout(&self) -> Option<Instant> {
        let peers = self.peers.values().flat_map(|p| p.conn.poll_timeout().into_iter().chain(p.open_by));
        peers.chain(self.gather.poll_timeout()).min()
    }

    pub fn poll_transmit(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        self.transmits.pop_front()
    }

    pub fn poll_event(&mut self) -> Option<HostEvent> {
        self.events.pop_front()
    }

    /// Queues one game message; dropped if the peer is gone.
    pub fn send(&mut self, peer: PeerId, msg: Bytes, now: Instant) {
        if let Some(p) = self.peers.get_mut(&peer) {
            p.conn.send(msg, now);
            self.pump(peer, now);
        }
    }

    /// See [`Connection::queued_bytes`].
    pub fn queued_bytes(&self, peer: PeerId) -> Option<usize> {
        self.peers.get(&peer).map(|p| p.conn.queued_bytes())
    }

    /// The first address the peer's datagrams came from, directly or through a relay.
    pub fn remote_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        self.peers.get(&peer)?.addrs.first().map(|(_, source)| *source)
    }

    /// Flushes what is queued and closes; [`HostEvent::Closed`] follows.
    pub fn close(&mut self, peer: PeerId, now: Instant) {
        if let Some(p) = self.peers.get_mut(&peer) {
            p.conn.close(now);
            self.pump(peer, now);
        }
    }

    fn drain_gather(&mut self) {
        self.transmits.extend(std::iter::from_fn(|| self.gather.poll_transmit()));
    }

    /// Moves a connection's output into the host's queues (relayed sends wrapped for their TURN
    /// server), and forgets it once it has closed.
    fn pump(&mut self, id: PeerId, now: Instant) {
        let Some(peer) = self.peers.get_mut(&id) else { return };
        while let Some(t) = peer.conn.poll_transmit() {
            if !self.gather.send(t.source, t.destination, &t.contents, now) {
                self.transmits.push_back((t.destination, t.contents));
            }
        }
        self.transmits.extend(std::iter::from_fn(|| self.gather.poll_transmit()));
        let mut closed = None;
        while let Some(event) = peer.conn.poll_event() {
            match event {
                Event::Open => {
                    peer.open_by = None;
                    self.events.push_back(HostEvent::Open(id));
                }
                Event::Closed(reason) => closed = Some(reason),
            }
        }
        while let Some(msg) = peer.conn.poll_message() {
            self.events.push_back(HostEvent::Message(id, msg));
        }
        if let Some(reason) = closed {
            let peer = self.peers.remove(&id).expect("just borrowed");
            self.by_ufrag.remove(peer.conn.local_ufrag());
            for path in peer.addrs {
                self.by_addr.remove(&path);
            }
            // Players behind one NAT share the address their signaling came from.
            for addr in peer.permits.into_iter().filter(|addr| !self.peers.values().any(|p| p.permits.contains(addr))) {
                self.gather.forget(addr, now);
            }
            self.events.push_back(HostEvent::Closed(id, reason));
        }
    }
}
