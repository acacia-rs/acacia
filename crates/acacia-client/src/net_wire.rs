//! A NetherNet connection plus the TURN allocation its relay candidate lives on, sharing one socket:
//! datagrams from the TURN server are unwrapped for the relay candidate, relayed sends get wrapped.

use std::net::SocketAddr;
use std::time::Instant;

use acacia_nethernet::turn::TurnClient;
use acacia_nethernet::Connection;

pub(crate) struct NetherNetWire {
    pub conn: Connection,
    turn: Option<TurnClient>,
    /// Remote candidate addresses, for permissions when TURN starts late.
    remote_addrs: Vec<SocketAddr>,
}

impl NetherNetWire {
    pub fn new(conn: Connection, turn: Option<TurnClient>) -> Self {
        Self { conn, turn, remote_addrs: Vec::new() }
    }

    pub fn turn(&self) -> Option<&TurnClient> {
        self.turn.as_ref()
    }

    pub fn turn_mut(&mut self) -> Option<&mut TurnClient> {
        self.turn.as_mut()
    }

    /// Starts relaying through `turn` (a late fallback), permitting every remote candidate seen so far.
    pub fn set_turn(&mut self, mut turn: TurnClient, now: Instant) {
        for peer in &self.remote_addrs {
            turn.create_permission(*peer, now);
        }
        self.turn = Some(turn);
    }

    /// Trickled remote candidates also need a TURN permission before the peer can reach our relay.
    pub fn add_remote_candidate(&mut self, line: &str, now: Instant) -> Result<(), acacia_nethernet::Error> {
        self.conn.add_remote_candidate(line, now)?;
        self.permit(line, now);
        Ok(())
    }

    /// Permissions for the candidates embedded in an SDP answer.
    pub fn permit_answer(&mut self, answer: &str, now: Instant) {
        for line in answer.lines().filter_map(|l| l.strip_prefix("a=")).filter(|l| l.starts_with("candidate:")) {
            self.permit(line, now);
        }
    }

    fn permit(&mut self, line: &str, now: Instant) {
        let Some(peer) = candidate_addr(line) else { return };
        self.remote_addrs.push(peer);
        if let Some(turn) = self.turn.as_mut() {
            turn.create_permission(peer, now);
        }
    }

    pub fn handle_datagram(&mut self, now: Instant, source: SocketAddr, data: &[u8]) {
        if let Some(turn) = self.turn.as_mut().filter(|t| t.server() == source) {
            let relayed = turn.handle_datagram(now, data);
            if let (Some(relay), Some((peer, payload))) = (turn.relayed(), relayed) {
                self.conn.handle_relayed(now, relay, peer, &payload);
            }
            return;
        }
        self.conn.handle_datagram(now, source, data);
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        self.conn.handle_timeout(now);
        if let Some(turn) = self.turn.as_mut().filter(|t| t.poll_timeout().is_some_and(|d| d <= now)) {
            turn.handle_timeout(now);
        }
    }

    pub fn poll_timeout(&self) -> Option<Instant> {
        self.conn.poll_timeout().into_iter().chain(self.turn.as_ref().and_then(TurnClient::poll_timeout)).min()
    }

    /// Next datagram for the socket and where it goes.
    pub fn poll_datagram(&mut self, now: Instant) -> Option<(SocketAddr, Vec<u8>)> {
        while let Some(t) = self.conn.poll_transmit() {
            match self.turn.as_mut() {
                Some(turn) if turn.relayed() == Some(t.source) => turn.send_to(t.destination, &t.contents, now),
                _ => return Some((t.destination, t.contents)),
            }
        }
        let turn = self.turn.as_mut()?;
        turn.poll_transmit().map(|d| (turn.server(), d))
    }

    pub fn close(&mut self, now: Instant) {
        self.conn.close(now);
        if let Some(turn) = self.turn.as_mut() {
            turn.close(now);
        }
    }
}

/// The address of a `candidate:<foundation> <component> <proto> <priority> <ip> <port> typ …` line.
fn candidate_addr(line: &str) -> Option<SocketAddr> {
    let mut f = line.split_whitespace().skip(4);
    Some(SocketAddr::new(f.next()?.parse().ok()?, f.next()?.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_candidate_addresses() {
        let line = "candidate:1 1 udp 2113937151 192.168.1.2 5000 typ host generation 0";
        assert_eq!(candidate_addr(line), Some("192.168.1.2:5000".parse().unwrap()));
        assert_eq!(candidate_addr("candidate:1 1 udp 1 host.local 5000 typ host"), None);
    }
}
