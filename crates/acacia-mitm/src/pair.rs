//! One player proxied over RakNet: the game (behind the RakNet server in main.rs), our connection to
//! the server, and the relay between them (relay.rs).

use std::net::SocketAddr;
use std::time::Instant;

use acacia_raknet::{self as raknet, Reliability};
use bytes::Bytes;
use serde_json::json;

use crate::record::Recorder;
use crate::relay::Relay;

pub struct Pair {
    upstream: raknet::Client,
    relay: Relay,
    /// Batches from the game that arrived before the upstream connection was up.
    pending: Vec<Bytes>,
    to_game: Vec<Bytes>,
    closed: bool,
}

impl Pair {
    pub fn new(server: SocketAddr, now: Instant, relay: Relay) -> Self {
        // go-raknet servers reject positive client GUIDs (DESIGN.md).
        let guid = rand_core::RngCore::next_u64(&mut rand_core::OsRng) | 1 << 63;
        Self {
            upstream: raknet::Client::new(raknet::Config::new(guid), server, now),
            relay,
            pending: Vec::new(),
            to_game: Vec::new(),
            closed: false,
        }
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

    pub fn handle_timeout(&mut self, now: Instant, rec: &mut Recorder) {
        self.upstream.handle_timeout(now);
        self.pump(rec);
    }

    pub fn on_upstream_datagram(&mut self, now: Instant, data: Bytes, rec: &mut Recorder) {
        self.upstream.handle_datagram(now, data);
        self.pump(rec);
    }

    /// The game left: say goodbye upstream (drain `poll_transmit` once more to send it).
    pub fn close(&mut self, now: Instant, rec: &mut Recorder) {
        self.upstream.close(now);
        self.pump(rec);
    }

    fn fail(&mut self, why: impl std::fmt::Display, rec: &mut Recorder) {
        eprintln!("closing: {why}");
        self.upstream.close(Instant::now());
        self.mark_closed(rec);
    }

    fn mark_closed(&mut self, rec: &mut Recorder) {
        if !std::mem::replace(&mut self.closed, true) {
            rec.write(json!({ "event": "closed" }));
        }
    }

    pub fn on_game_message(&mut self, msg: &[u8], rec: &mut Recorder) {
        match self.relay.on_game_message(msg, rec) {
            Ok(batch) => {
                if !self.pending.is_empty() || !self.upstream.send(batch.clone(), Reliability::ReliableOrdered) {
                    self.pending.push(batch);
                }
            }
            Err(e) => self.fail(e, rec),
        }
    }

    fn pump(&mut self, rec: &mut Recorder) {
        while let Some(event) = self.upstream.poll_event() {
            match event {
                raknet::Event::Connected { .. } => {
                    for batch in std::mem::take(&mut self.pending) {
                        self.upstream.send(batch, Reliability::ReliableOrdered);
                    }
                }
                raknet::Event::Message(msg) => match self.relay.on_server_message(&msg, rec) {
                    Ok(batches) => self.to_game.extend(batches),
                    Err(e) => self.fail(e, rec),
                },
                raknet::Event::Disconnected(reason) => {
                    println!("server connection ended: {reason:?}");
                    self.mark_closed(rec);
                }
            }
        }
    }
}
