//! Many answering connections behind one UDP socket: a network-free NetherNet host. Signaling
//! (HTTP, LAN) hands it offers; it routes each datagram to its connection, by the ICE username of
//! binding requests and by source address after that. A datagram it does not claim belongs to
//! whatever else shares the socket (RakNet).

mod route;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::Bytes;
use p384::ecdsa::SigningKey;

use crate::{Connection, Error, Event, LocalCandidate};

/// Source addresses one peer may hold; ICE settles on one, a second network path adds a few.
const MAX_ADDRS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerId(pub u64);

#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Connections held at once, opening ones included.
    pub max_peers: usize,
    /// From the answer until both data channels are open.
    pub handshake_timeout: Duration,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self { max_peers: 1024, handshake_timeout: Duration::from_secs(15) }
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

struct Peer {
    conn: Connection,
    addrs: Vec<SocketAddr>,
    /// Until the channels open.
    open_by: Option<Instant>,
}

pub struct Host {
    /// Signs every answer; clients pin it per server address.
    key: SigningKey,
    config: HostConfig,
    peers: HashMap<PeerId, Peer>,
    by_ufrag: HashMap<String, PeerId>,
    by_addr: HashMap<SocketAddr, PeerId>,
    next_id: u64,
    transmits: VecDeque<(SocketAddr, Vec<u8>)>,
    events: VecDeque<HostEvent>,
}

impl Host {
    pub fn new(key: SigningKey, config: HostConfig) -> Self {
        Self {
            key,
            config,
            peers: HashMap::new(),
            by_ufrag: HashMap::new(),
            by_addr: HashMap::new(),
            next_id: 1,
            transmits: VecDeque::new(),
            events: VecDeque::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    /// Answers a direct-connect offer with `local` (the socket, as the client can reach it) embedded.
    pub fn answer(&mut self, offer: &str, local: SocketAddr, now: Instant) -> Result<Answer, Error> {
        let (conn, sdp) = self.unique(|key| Connection::answer(offer, local, key, now))?;
        Ok(Answer { peer: self.insert(conn, now), sdp, candidates: Vec::new() })
    }

    /// Answers a `CONNECTREQUEST`; `local` goes out as a trickled candidate.
    pub fn trickle_answer(&mut self, offer: &str, local: SocketAddr, now: Instant) -> Result<Answer, Error> {
        let (mut conn, sdp) = self.unique(|key| Connection::trickle_answer(offer, key, now))?;
        let candidate = conn.add_local_candidate(LocalCandidate::Host(local), now)?;
        Ok(Answer { peer: self.insert(conn, now), sdp, candidates: vec![candidate] })
    }

    /// Builds a connection whose ICE ufrag no other peer has: datagrams are routed by it.
    fn unique(&self, build: impl Fn(&SigningKey) -> Result<(Connection, String), Error>) -> Result<(Connection, String), Error> {
        if self.peers.len() >= self.config.max_peers {
            return Err(Error::HostFull);
        }
        loop {
            let built = build(&self.key)?;
            if !self.by_ufrag.contains_key(built.0.local_ufrag()) {
                return Ok(built);
            }
        }
    }

    fn insert(&mut self, conn: Connection, now: Instant) -> PeerId {
        let id = PeerId(self.next_id);
        self.next_id += 1;
        self.by_ufrag.insert(conn.local_ufrag().to_owned(), id);
        self.peers.insert(id, Peer { conn, addrs: Vec::new(), open_by: Some(now + self.config.handshake_timeout) });
        self.pump(id);
        id
    }

    /// A trickled `candidate:` line from the peer.
    pub fn add_remote_candidate(&mut self, peer: PeerId, line: &str, now: Instant) -> Result<(), Error> {
        let Some(p) = self.peers.get_mut(&peer) else { return Ok(()) };
        p.conn.add_remote_candidate(line, now)?;
        self.pump(peer);
        Ok(())
    }

    /// Takes a datagram from the shared socket. False if it belongs to no connection here.
    pub fn handle_datagram(&mut self, now: Instant, source: SocketAddr, data: &[u8]) -> bool {
        let Some(id) = self.route(now, source, data) else { return false };
        if let Some(peer) = self.peers.get_mut(&id) {
            peer.conn.handle_datagram(now, source, data);
        }
        self.pump(id);
        true
    }

    fn route(&mut self, now: Instant, source: SocketAddr, data: &[u8]) -> Option<PeerId> {
        let known = self.by_addr.get(&source).copied();
        // A binding request names its connection, which beats a stale mapping when an address is
        // reused; its credentials are checked first, so a spoofed one cannot take an address over.
        let named = route::request_ufrag(data).and_then(|ufrag| self.by_ufrag.get(ufrag).copied());
        let Some(id) = named.filter(|id| known != Some(*id)) else { return known };
        let peer = self.peers.get_mut(&id)?;
        if peer.addrs.len() >= MAX_ADDRS || !peer.conn.accepts(now, source, data) {
            return known;
        }
        peer.addrs.push(source);
        if let Some(old) = self.by_addr.insert(source, id).and_then(|old| self.peers.get_mut(&old)) {
            old.addrs.retain(|a| *a != source);
        }
        Some(id)
    }

    pub fn handle_timeout(&mut self, now: Instant) {
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
            self.pump(id);
        }
    }

    pub fn poll_timeout(&self) -> Option<Instant> {
        self.peers.values().flat_map(|p| p.conn.poll_timeout().into_iter().chain(p.open_by)).min()
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
            self.pump(peer);
        }
    }

    /// See [`Connection::queued_bytes`].
    pub fn queued_bytes(&self, peer: PeerId) -> Option<usize> {
        self.peers.get(&peer).map(|p| p.conn.queued_bytes())
    }

    /// The first address the peer's datagrams came from.
    pub fn remote_addr(&self, peer: PeerId) -> Option<SocketAddr> {
        self.peers.get(&peer)?.addrs.first().copied()
    }

    /// Flushes what is queued and closes; [`HostEvent::Closed`] follows.
    pub fn close(&mut self, peer: PeerId, now: Instant) {
        if let Some(p) = self.peers.get_mut(&peer) {
            p.conn.close(now);
            self.pump(peer);
        }
    }

    /// Moves a connection's output into the host's queues, and forgets it once it has closed.
    fn pump(&mut self, id: PeerId) {
        let Some(peer) = self.peers.get_mut(&id) else { return };
        while let Some(t) = peer.conn.poll_transmit() {
            self.transmits.push_back((t.destination, t.contents));
        }
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
            for addr in peer.addrs {
                self.by_addr.remove(&addr);
            }
            self.events.push_back(HostEvent::Closed(id, reason));
        }
    }
}
