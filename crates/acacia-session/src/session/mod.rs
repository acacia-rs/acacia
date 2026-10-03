mod blob_cache;
mod deferred;
mod handlers;
mod link;

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use acacia_proto::packets::{Disconnect, PlayStatusStatus, RequestNetworkSettings};
use acacia_proto::types::DisconnectFailReason;
use acacia_proto::{encode_packet, Packet, RawPacket, PROTOCOL_VERSION};
use acacia_raknet as raknet;
use bytes::{Bytes, BytesMut};
use p384::ecdsa::SigningKey;

use crate::batch::BatchCodec;
use crate::blob_store::BlobStore;
use crate::Error;
use blob_cache::BlobStatus;
use deferred::Deferred;
use link::Link;
pub use link::LinkConfig;

pub struct SessionConfig {
    pub link: LinkConfig,
    /// Client key; must be the key `login_request` was signed with.
    pub key: SigningKey,
    /// Login `tokens` payload from `acacia_auth::build_connection_request` (or the offline variant).
    pub login_request: Vec<u8>,
    pub chunk_radius: i32,
    /// Respawn automatically when the player dies after spawning.
    pub auto_respawn: bool,
    /// Send SetLocalPlayerAsInitialized as soon as the player spawns. Off when the caller plays
    /// vanilla's loading-screen sequence and sends it itself (acacia-bot does).
    pub initialize_on_spawn: bool,
    /// Report the client blob cache as enabled and answer with this store; `None` disables it (blob_cache.rs).
    pub blob_store: Option<Arc<dyn BlobStore>>,
}

#[derive(Debug)]
pub enum Event {
    /// Login finished and the player is in the world.
    Spawned { runtime_entity_id: u64 },
    /// Every packet from the server, including the ones the session also handled internally.
    Packet(RawPacket),
    Disconnected(DisconnectReason),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DisconnectReason {
    Transport(raknet::DisconnectReason),
    /// The NetherNet (WebRTC) connection failed or closed; reported by the I/O driver.
    NetherNet(String),
    Kicked { reason: String, message: String },
    /// The server moved the player elsewhere; reconnect to this address to follow it.
    Transfer { address: String, port: u16 },
    LoginFailed(PlayStatusStatus),
    Protocol(String),
    LocalClose,
    /// Socket failure reported by the I/O driver.
    Io(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Connecting,
    LoggingIn,
    Spawned,
    Closed,
}

/// A network-free Bedrock client session: batching, encryption and the login sequence over a
/// RakNet or message [`LinkConfig`].
pub struct Session {
    link: Link,
    codec: BatchCodec,
    stage: Stage,
    key: SigningKey,
    login_request: Vec<u8>,
    chunk_radius: i32,
    auto_respawn: bool,
    initialize_on_spawn: bool,
    /// The server has sent `Respawn(ready)`; "searching" before it belongs to joining (handlers.rs).
    respawn_ready_seen: bool,
    /// We asked to respawn and await the server's `Respawn(ready)` (handlers.rs).
    respawn_requested: bool,
    runtime_entity_id: Option<u64>,
    outgoing: Vec<Bytes>,
    /// Replies waiting out a vanilla reaction time (deferred.rs).
    deferred: Deferred,
    blobs: Option<BlobStatus>,
    /// The time of the event being handled, for `send_later`.
    now: Instant,
    events: VecDeque<Event>,
    scratch: Vec<Bytes>,
}

impl Session {
    pub fn new(cfg: SessionConfig, server: SocketAddr, now: Instant) -> Self {
        let link = Link::new(cfg.link, server, now);
        let codec = if link.is_message() { BatchCodec::without_header() } else { BatchCodec::default() };
        let mut session = Self {
            link,
            codec,
            stage: Stage::Connecting,
            key: cfg.key,
            login_request: cfg.login_request,
            chunk_radius: cfg.chunk_radius,
            auto_respawn: cfg.auto_respawn,
            initialize_on_spawn: cfg.initialize_on_spawn,
            respawn_ready_seen: false,
            respawn_requested: false,
            runtime_entity_id: None,
            outgoing: Vec::new(),
            deferred: Deferred::new(),
            blobs: cfg.blob_store.map(BlobStatus::new),
            now,
            events: VecDeque::new(),
            scratch: Vec::new(),
        };
        session.pump(now);
        session
    }

    pub fn is_spawned(&self) -> bool {
        self.stage == Stage::Spawned
    }

    pub fn rtt(&self) -> Option<std::time::Duration> {
        self.link.rtt()
    }

    pub fn send<T: Packet>(&mut self, packet: &T) {
        let mut buf = BytesMut::new();
        encode_packet(packet, &mut buf);
        self.outgoing.push(buf.freeze());
    }

    /// Queues an already-encoded packet (header + body).
    pub fn send_raw(&mut self, packet: Bytes) {
        self.outgoing.push(packet);
    }

    /// Sends `packet` after a random delay in `delay_ms`, in order with other deferred packets.
    fn send_later<T: Packet>(&mut self, delay_ms: (u64, u64), packet: &T) {
        let mut buf = BytesMut::new();
        encode_packet(packet, &mut buf);
        self.deferred.push(self.now, delay_ms, buf.freeze());
    }

    pub fn close(&mut self, now: Instant) {
        self.finish(now, DisconnectReason::LocalClose);
    }

    /// A RakNet datagram from the server.
    pub fn handle_datagram(&mut self, now: Instant, data: Bytes) {
        self.link.handle_datagram(now, data);
        self.pump(now);
    }

    /// A whole game message from a message link (one reassembled NetherNet message).
    pub fn handle_message(&mut self, now: Instant, msg: Bytes) {
        self.link.handle_message(msg);
        self.pump(now);
    }

    pub fn handle_timeout(&mut self, now: Instant) {
        self.link.handle_timeout(now);
        while let Some(packet) = self.deferred.pop_due(now) {
            self.outgoing.push(packet);
        }
        if let Some(status) = self.blobs.as_mut().and_then(|b| b.poll(now)) {
            self.send(&status);
        }
        self.pump(now);
    }

    pub fn poll_timeout(&self) -> Option<Instant> {
        let blobs = self.blobs.as_ref().and_then(BlobStatus::next_due);
        self.link.poll_timeout().into_iter().chain(self.deferred.next_due()).chain(blobs).min()
    }

    /// Batches queued packets into one game message, then yields RakNet datagrams to send, or
    /// whole game messages on a message link.
    pub fn poll_transmit(&mut self, now: Instant) -> Option<Bytes> {
        self.flush();
        self.link.poll_transmit(now)
    }

    pub fn poll_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    fn flush(&mut self) {
        if self.outgoing.is_empty() || !self.link.is_connected() {
            return;
        }
        let batch = self.codec.encode(self.outgoing.iter().map(|p| &p[..]));
        self.outgoing.clear();
        self.link.send(batch);
    }

    fn finish(&mut self, now: Instant, reason: DisconnectReason) {
        if self.stage == Stage::Closed {
            return;
        }
        // RakNet tells the server itself; a message link needs the game-level goodbye, or the
        // server keeps the player until it times out (and kicks the rejoin with ServerIdConflict).
        if reason == DisconnectReason::LocalClose && self.link.is_message() {
            self.send(&Disconnect {
                reason: DisconnectFailReason::Disconnected,
                hide_disconnect_reason: true,
                content: None,
            });
        }
        self.stage = Stage::Closed;
        self.flush();
        self.link.close(now);
        self.events.push_back(Event::Disconnected(reason));
    }

    fn pump(&mut self, now: Instant) {
        self.now = now;
        while let Some(event) = self.link.poll_event() {
            let result = match event {
                raknet::Event::Connected { .. } => {
                    self.stage = Stage::LoggingIn;
                    self.send(&RequestNetworkSettings { client_protocol: PROTOCOL_VERSION });
                    Ok(None)
                }
                raknet::Event::Message(msg) => self.handle_batch(&msg),
                raknet::Event::Disconnected(reason) => {
                    if self.stage != Stage::Closed {
                        self.stage = Stage::Closed;
                        self.events.push_back(Event::Disconnected(DisconnectReason::Transport(reason)));
                    }
                    Ok(None)
                }
            };
            match result {
                Ok(Some(reason)) => self.finish(now, reason),
                Err(e) => self.finish(now, DisconnectReason::Protocol(e.to_string())),
                Ok(None) => {}
            }
            if self.stage == Stage::Closed {
                return;
            }
        }
    }

    fn handle_batch(&mut self, msg: &[u8]) -> Result<Option<DisconnectReason>, Error> {
        let mut packets = std::mem::take(&mut self.scratch);
        self.codec.decode(msg, &mut packets)?;
        let mut outcome = Ok(None);
        for buf in packets.drain(..) {
            outcome = RawPacket::parse(buf).map_err(Error::from).and_then(|raw| self.handle_packet(raw));
            if !matches!(outcome, Ok(None)) {
                break;
            }
        }
        packets.clear();
        self.scratch = packets;
        outcome
    }
}
