//! One player whose game joined over RakNet (behind the RakNet server in raknet.rs): our
//! connection to the server, and the relay between the two (relay.rs). The server is reached over
//! RakNet too, or, for a realm, over a raw link whose batches raknet.rs carries to and from here.

use std::net::SocketAddr;
use std::time::Instant;

use acacia_raknet::{self as raknet, Reliability};
use bytes::Bytes;
use tokio::sync::mpsc;

use crate::intercept::Direction;
use crate::relay::{Out, Relay};

enum Upstream {
    RakNet(raknet::Client),
    /// Batches for the link; `None` once we have closed it.
    Raw(Option<mpsc::UnboundedSender<Bytes>>),
}

pub struct Pair {
    upstream: Upstream,
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
        Self::over(Upstream::RakNet(raknet::Client::new(raknet::Config::new(guid), server, now)), relay)
    }

    /// Over a link that is already open: what goes into `link` reaches the server as one batch.
    pub fn raw(link: mpsc::UnboundedSender<Bytes>, relay: Relay) -> Self {
        Self::over(Upstream::Raw(Some(link)), relay)
    }

    fn over(upstream: Upstream, relay: Relay) -> Self {
        Self { upstream, relay, pending: Vec::new(), to_game: Vec::new(), transfer: None, closed: false }
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
        match &mut self.upstream {
            Upstream::RakNet(client) => client.poll_transmit(now),
            Upstream::Raw(_) => None,
        }
    }

    pub fn poll_timeout(&self) -> Option<Instant> {
        match &self.upstream {
            Upstream::RakNet(client) => client.poll_timeout(),
            Upstream::Raw(_) => None,
        }
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        if let Upstream::RakNet(client) = &mut self.upstream {
            client.handle_timeout(now);
            self.pump();
        }
    }

    /// What arrived from the server: a datagram over RakNet, a whole batch over a raw link.
    pub fn on_upstream(&mut self, now: Instant, data: Bytes) {
        match &mut self.upstream {
            Upstream::RakNet(client) => {
                client.handle_datagram(now, data);
                self.pump();
            }
            Upstream::Raw(_) => self.on_server_message(&data),
        }
    }

    /// The raw link ended on the server's side.
    pub fn on_upstream_ended(&mut self) {
        println!("server connection ended");
        self.upstream_close();
        self.mark_closed();
    }

    /// The game left: say goodbye upstream (drain `poll_transmit` once more to send it).
    pub fn close(&mut self, now: Instant) {
        match &mut self.upstream {
            Upstream::RakNet(client) => {
                client.close(now);
                self.pump();
            }
            Upstream::Raw(link) => {
                if let (Some(link), Some(bye)) = (link.as_ref(), self.relay.goodbye()) {
                    let _ = link.send(bye);
                }
                self.upstream_close();
                self.mark_closed();
            }
        }
    }

    fn upstream_close(&mut self) {
        match &mut self.upstream {
            Upstream::RakNet(client) => client.close(Instant::now()),
            // Dropping the sender ends the link once what is queued has gone out (raknet.rs).
            Upstream::Raw(link) => *link = None,
        }
    }

    fn fail(&mut self, why: impl std::fmt::Display) {
        eprintln!("closing: {why}");
        self.upstream_close();
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

    fn on_server_message(&mut self, msg: &[u8]) {
        match self.relay.on_server_message(msg) {
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
            match &mut self.upstream {
                Upstream::RakNet(client) => {
                    if !self.pending.is_empty() || !client.send(batch.clone(), Reliability::ReliableOrdered) {
                        self.pending.push(batch);
                    }
                }
                Upstream::Raw(Some(link)) => drop(link.send(batch)),
                Upstream::Raw(None) => {}
            }
        }
        self.to_game.extend(out.to_game);
        self.transfer = out.transfer.or(self.transfer.take());
    }

    fn pump(&mut self) {
        let Upstream::RakNet(client) = &mut self.upstream else { return };
        let mut messages = Vec::new();
        while let Some(event) = client.poll_event() {
            match event {
                raknet::Event::Connected { .. } => {
                    for batch in std::mem::take(&mut self.pending) {
                        client.send(batch, Reliability::ReliableOrdered);
                    }
                }
                raknet::Event::Message(msg) => messages.push(msg),
                raknet::Event::Disconnected(reason) => {
                    println!("server connection ended: {reason:?}");
                    if !std::mem::replace(&mut self.closed, true) {
                        self.relay.close();
                    }
                }
            }
        }
        messages.iter().for_each(|msg| self.on_server_message(msg));
    }
}
