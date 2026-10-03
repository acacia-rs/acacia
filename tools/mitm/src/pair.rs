//! One proxied player: the game (behind the RakNet server in main.rs) and our connection to the
//! server. Only the encryption handshake is terminated, each side getting its own key; every other
//! packet passes through unchanged, batched as it came, so the game's own replies (packs, cache
//! status, chunk radius) are what the server sees.

use std::net::SocketAddr;
use std::time::Instant;

use acacia_session::batch::BatchCodec;
use acacia_session::compression::Algorithm;
use acacia_session::crypto::derive_key;
use acacia_proto::packets::{Login, NetworkSettings, PlayStatus, PlayStatusStatus, ServerToClientHandshake};
use acacia_proto::{Packet, RawPacket};
use acacia_raknet::{self as raknet, Reliability};
use bytes::Bytes;
use p384::ecdsa::SigningKey;
use serde_json::json;

use crate::login;
use crate::record::Recorder;

pub struct Pair {
    upstream: raknet::Client,
    game_codec: BatchCodec,
    up_codec: BatchCodec,
    /// The proxy's key on both sides: it signs the upstream login and the game-side handshake.
    key: SigningKey,
    game_key: Option<p384::PublicKey>,
    /// Batches from the game that arrived before the upstream connection was up.
    pending: Vec<Bytes>,
    to_game: Vec<Bytes>,
    closed: bool,
}

impl Pair {
    pub fn new(server: SocketAddr, now: Instant) -> Self {
        // go-raknet servers reject positive client GUIDs (DESIGN.md).
        let guid = rand_core::RngCore::next_u64(&mut rand_core::OsRng) | 1 << 63;
        Self {
            upstream: raknet::Client::new(raknet::Config::new(guid), server, now),
            game_codec: BatchCodec::default(),
            up_codec: BatchCodec::default(),
            key: SigningKey::random(&mut rand_core::OsRng),
            game_key: None,
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
        let mut packets = Vec::new();
        if let Err(e) = self.game_codec.decode(msg, &mut packets) {
            return self.fail(format_args!("game batch: {e}"), rec);
        }
        let mut forward = Vec::with_capacity(packets.len());
        for buf in packets {
            let raw = match RawPacket::parse(buf.clone()) {
                Ok(raw) => raw,
                Err(e) => return self.fail(format_args!("game packet header: {e}"), rec),
            };
            if raw.id != Login::ID {
                rec.packet(true, &raw);
                forward.push(buf);
                continue;
            }
            match login::read(&raw.body, &self.key) {
                Ok(l) => {
                    rec.login(raw.body.len(), l.summary);
                    rec.write(json!({ "event": "login", "client_data": login::trimmed(&l.client_data), "identity": l.identity }));
                    rec.save_skin(&l.client_data);
                    println!("{} logging in", l.identity["DisplayName"]);
                    self.game_key = Some(l.game_key);
                    forward.push(l.upstream);
                }
                Err(e) => return self.fail(format_args!("game login: {e}"), rec),
            }
        }
        let batch = self.up_codec.encode(forward.iter().map(|b| &b[..]));
        if !self.pending.is_empty() || !self.upstream.send(batch.clone(), Reliability::ReliableOrdered) {
            self.pending.push(batch);
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
                raknet::Event::Message(msg) => {
                    if let Err(e) = self.on_server_batch(&msg, rec) {
                        self.fail(e, rec);
                    }
                }
                raknet::Event::Disconnected(reason) => {
                    println!("server connection ended: {reason:?}");
                    self.mark_closed(rec);
                }
            }
        }
    }

    fn on_server_batch(&mut self, msg: &[u8], rec: &mut Recorder) -> Result<(), String> {
        let mut packets = Vec::new();
        self.up_codec.decode(msg, &mut packets).map_err(|e| format!("server batch: {e}"))?;
        let mut forward = Vec::with_capacity(packets.len());
        for buf in packets {
            let raw = RawPacket::parse(buf.clone()).map_err(|e| format!("server packet header: {e}"))?;
            match raw.id {
                NetworkSettings::ID => {
                    rec.packet(false, &raw);
                    forward.push(buf);
                    self.send_to_game(&mut forward);
                    let settings: NetworkSettings = raw.decode().map_err(|e| e.to_string())?;
                    let alg = Algorithm::from_settings(settings.compression_algorithm).map_err(|e| e.to_string())?;
                    let threshold = settings.compression_threshold.into();
                    self.game_codec.enable_compression(alg, threshold);
                    self.up_codec.enable_compression(alg, threshold);
                }
                ServerToClientHandshake::ID => {
                    let handshake: ServerToClientHandshake = raw.decode().map_err(|e| e.to_string())?;
                    let (server_key, salt) = acacia_auth::parse_server_handshake(&handshake.token).map_err(|e| e.to_string())?;
                    self.up_codec.enable_encryption(derive_key(&self.key, &server_key, &salt));
                    let game_key = self.game_key.as_ref().ok_or("server handshake before the game's login")?;
                    let (ours, session_key) = login::handshake_for_game(&self.key, game_key);
                    rec.packet(false, &RawPacket::parse(ours.clone()).expect("encoded above"));
                    forward.push(ours);
                    self.send_to_game(&mut forward);
                    self.game_codec.enable_encryption(session_key);
                }
                _ => {
                    if raw.decode::<PlayStatus>().is_ok_and(|s| s.status == PlayStatusStatus::PlayerSpawn) {
                        rec.write(json!({ "event": "spawned" }));
                    }
                    rec.packet(false, &raw);
                    forward.push(buf);
                }
            }
        }
        self.send_to_game(&mut forward);
        Ok(())
    }

    /// Encodes `packets` as one game batch now, before a codec change applies to later ones.
    fn send_to_game(&mut self, packets: &mut Vec<Bytes>) {
        if !packets.is_empty() {
            self.to_game.push(self.game_codec.encode(packets.iter().map(|b| &b[..])));
            packets.clear();
        }
    }
}
