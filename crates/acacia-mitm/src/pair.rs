//! One player proxied over RakNet: the game (behind the RakNet server in raknet.rs), our connection
//! to the server, and the relay between them (relay.rs).

use std::net::SocketAddr;
use std::time::Instant;

use acacia_raknet::{self as raknet, Reliability};
use bytes::Bytes;

use crate::intercept::Direction;
use crate::relay::{Out, Relay};

pub struct Pair {
    upstream: raknet::Client,
    relay: Relay,
    /// Batches for the server that came before the upstream connection was up.
    pending: Vec<Bytes>,
    to_game: Vec<Bytes>,
    transfer: Option<(String, u16)>,
    closed: bool,
}

impl Pair {
    pub fn new(server: SocketAddr, now: Instant, relay: Relay) -> Self {
        // go-raknet servers reject positive client GUIDs (DESIGN.md).
        let guid = rand_core::RngCore::next_u64(&mut rand_core::OsRng) | 1 << 63;
        Self { upstream: raknet::Client::new(raknet::Config::new(guid), server, now), relay, pending: Vec::new(), to_game: Vec::new(), transfer: None, closed: false }
    }

    /// Where the server just transferred the game, once.
    pub fn take_transfer(&mut self) -> Option<(String, u16)> {
        self.transfer.take()
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Game batches to hand to the RakNet server.
    pub fn take_to_game(&mut self) -> Vec<Bytes> {
        std::mem::take(&mut self.to_game)
    }

    pub fn poll_transmit(&mut self, now: Instant) -> Option<Bytes> {
        self.upstream.poll_transmit(now)
    }

    pub fn poll_timeout(&self) -> Option<Instant> {
        self.upstream.poll_timeout()
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        self.upstream.handle_timeout(now);
        self.pump();
    }

    pub fn on_upstream_datagram(&mut self, now: Instant, data: Bytes) {
        self.upstream.handle_datagram(now, data);
        self.pump();
    }

    /// The game left: say goodbye upstream (drain `poll_transmit` once more to send it).
    pub fn close(&mut self, now: Instant) {
        self.upstream.close(now);
        self.pump();
    }

    fn fail(&mut self, why: impl std::fmt::Display) {
        eprintln!("closing: {why}");
        self.upstream.close(Instant::now());
        self.mark_closed();
    }

    fn mark_closed(&mut self) {
        if !std::mem::replace(&mut self.closed, true) {
            self.relay.close();
        }
    }

    pub fn on_game_message(&mut self, msg: &[u8]) {
        match self.relay.on_game_message(msg) {
            Ok(out) => self.route(out),
            Err(e) => self.fail(e),
        }
    }

    pub fn inject(&mut self, packets: Vec<(Direction, Bytes)>) {
        let out = self.relay.inject(packets);
        self.route(out);
    }

    fn route(&mut self, out: Out) {
        for batch in out.to_server {
            if !self.pending.is_empty() || !self.upstream.send(batch.clone(), Reliability::ReliableOrdered) {
                self.pending.push(batch);
            }
        }
        self.to_game.extend(out.to_game);
        self.transfer = out.transfer.or(self.transfer.take());
    }

    fn pump(&mut self) {
        while let Some(event) = self.upstream.poll_event() {
            match event {
                raknet::Event::Connected { .. } => {
                    for batch in std::mem::take(&mut self.pending) {
                        self.upstream.send(batch, Reliability::ReliableOrdered);
                    }
                }
                raknet::Event::Message(msg) => match self.relay.on_server_message(&msg) {
                    Ok(out) => self.route(out),
                    Err(e) => self.fail(e),
                },
                raknet::Event::Disconnected(reason) => {
                    println!("server connection ended: {reason:?}");
                    self.mark_closed();
                }
            }
        }
    }
}
