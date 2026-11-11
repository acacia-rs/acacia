//! Offers in: each answered by a new connection, with the host's candidates, and the peer's
//! addresses let through our TURN relays.

use std::net::SocketAddr;
use std::time::Instant;

use p384::ecdsa::SigningKey;

use super::{route, Answer, Host, Peer, PeerId, MAX_PERMITS};
use crate::{Connection, Error, LocalCandidate};

impl Host {
    /// Answers a direct-connect offer with `local` (the socket, as the client can reach it) and the
    /// gathered candidates embedded.
    pub fn answer(&mut self, offer: &str, local: SocketAddr, now: Instant) -> Result<Answer, Error> {
        let extra = self.gather.candidates(local);
        let (conn, sdp) = self.unique(|key| Connection::answer_with(offer, local, &extra, key, now))?;
        let peer = self.insert(conn, now);
        for addr in route::candidate_addrs(offer) {
            self.permit(peer, addr, now);
        }
        Ok(Answer { peer, sdp, candidates: Vec::new() })
    }

    /// Answers a `CONNECTREQUEST`; `local` and the gathered candidates go out trickled.
    pub fn trickle_answer(&mut self, offer: &str, local: SocketAddr, now: Instant) -> Result<Answer, Error> {
        let (mut conn, sdp) = self.unique(|key| Connection::trickle_answer(offer, key, now))?;
        let mut candidates = vec![conn.add_local_candidate(LocalCandidate::Host(local), now)?];
        for extra in self.gather.candidates(local) {
            candidates.push(conn.add_local_candidate(extra, now)?);
        }
        Ok(Answer { peer: self.insert(conn, now), sdp, candidates })
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
        let open_by = Some(now + self.config.handshake_timeout);
        self.peers.insert(id, Peer { conn, addrs: Vec::new(), permits: Vec::new(), open_by });
        self.pump(id, now);
        id
    }

    /// Lets `addr` reach our relay candidates for `peer`. Offer and trickled candidates are permitted
    /// already; a client behind NAT offers only private ones, so pass where its signaling came from.
    pub fn permit(&mut self, peer: PeerId, addr: SocketAddr, now: Instant) {
        let Some(p) = self.peers.get_mut(&peer) else { return };
        if p.permits.len() < MAX_PERMITS && !p.permits.contains(&addr) {
            p.permits.push(addr);
            self.gather.permit(addr, now);
            self.drain_gather();
        }
    }

    /// A trickled `candidate:` line from the peer.
    pub fn add_remote_candidate(&mut self, peer: PeerId, line: &str, now: Instant) -> Result<(), Error> {
        let Some(p) = self.peers.get_mut(&peer) else { return Ok(()) };
        p.conn.add_remote_candidate(line, now)?;
        for addr in route::candidate_addrs(line) {
            self.permit(peer, addr, now);
        }
        self.pump(peer, now);
        Ok(())
    }
}
