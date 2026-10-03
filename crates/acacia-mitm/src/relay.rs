//! The game-level half of one proxied player, whatever the transport. Only the encryption handshake
//! is terminated, each side getting its own key. Every other packet is recorded as it came, then
//! goes through the player's interceptors (intercept.rs) and is re-batched for the other side with
//! that side's compression and encryption; with no interceptors the batches carry what came in.
//! Over NetherNet both sides still get a handshake but stay plaintext, as with BDS.

use acacia_auth::LoginCredentials;
use acacia_proto::packets::{
    ClientToServerHandshake, Disconnect, Login, NetworkSettings, PlayStatus, PlayStatusStatus, ServerToClientHandshake,
};
use acacia_proto::types::DisconnectFailReason;
use acacia_proto::{encode_packet, Packet, RawPacket};
use acacia_session::batch::BatchCodec;
use acacia_session::compression::Algorithm;
use acacia_session::crypto::derive_key;
use acacia_session::server::ServerConnection;
use bytes::{Bytes, BytesMut};
use p384::ecdsa::SigningKey;
use serde_json::{json, Value};

use crate::intercept::{Chain, Direction};
use crate::login;
use crate::record::{self, SharedRecorder};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    RakNet,
    NetherNet,
}

/// Encoded batches to send, in order, per side.
#[derive(Debug, Default)]
pub struct Out {
    pub to_server: Vec<Bytes>,
    pub to_game: Vec<Bytes>,
}

pub struct Relay {
    wire: Wire,
    game: ServerConnection,
    up_codec: BatchCodec,
    /// The proxy's key on both sides: it signs the upstream login and the game-side handshake.
    key: SigningKey,
    /// Online login for the server, bound to `key`; offline without.
    credentials: Option<LoginCredentials>,
    game_key: Option<p384::PublicKey>,
    game_sent_disconnect: bool,
    rec: Option<SharedRecorder>,
    chain: Chain,
    game_ready: bool,
    server_ready: bool,
    /// Injections waiting for their direction's handshake.
    held: Vec<(Direction, Bytes)>,
}

impl Relay {
    pub fn new(wire: Wire, key: SigningKey, credentials: Option<LoginCredentials>, rec: Option<SharedRecorder>, chain: Chain) -> Self {
        let (game, up_codec) = match wire {
            Wire::RakNet => (ServerConnection::new(key.clone()), BatchCodec::default()),
            Wire::NetherNet => (ServerConnection::nethernet(key.clone()), BatchCodec::without_header()),
        };
        Self {
            wire,
            game,
            up_codec,
            key,
            credentials,
            game_key: None,
            game_sent_disconnect: false,
            rec,
            chain,
            game_ready: false,
            server_ready: false,
            held: Vec::new(),
        }
    }

    /// Writes an event line to the capture, if recording.
    pub fn note(&self, entry: Value) {
        if let Some(rec) = &self.rec {
            record::lock(rec).write(entry);
        }
    }

    fn record(&self, from_game: bool, raw: &RawPacket) {
        if let Some(rec) = &self.rec {
            record::lock(rec).packet(from_game, raw);
        }
    }

    /// Records a game message and re-batches what the interceptors let through for the server,
    /// Login re-signed.
    pub fn on_game_message(&mut self, msg: &[u8]) -> Result<Out, String> {
        let packets = self.game.decode(msg).map_err(|e| format!("game batch: {e}"))?;
        let mut forward = Vec::with_capacity(packets.len());
        for buf in packets {
            let raw = RawPacket::parse(buf.clone()).map_err(|e| format!("game packet header: {e}"))?;
            if raw.id == Login::ID {
                forward.push(self.login(&raw)?);
                continue;
            }
            self.game_sent_disconnect |= raw.id == Disconnect::ID;
            self.server_ready |= raw.id == ClientToServerHandshake::ID;
            self.record(true, &raw);
            self.chain.run(true, buf, &raw, &mut forward).map_err(|e| format!("intercepted packet: {e}"))?;
        }
        let mut out = Out::default();
        if !forward.is_empty() {
            out.to_server.push(self.up_codec.encode(forward.iter().map(|b| &b[..])));
        }
        self.release_held(&mut out);
        Ok(out)
    }

    fn login(&mut self, raw: &RawPacket) -> Result<Bytes, String> {
        let l = login::read(&raw.body, &self.key, self.credentials.as_ref()).map_err(|e| format!("game login: {e}"))?;
        if let Some(rec) = &self.rec {
            let mut rec = record::lock(rec);
            rec.login(raw.body.len(), l.summary);
            rec.write(json!({ "event": "login", "client_data": login::trimmed(&l.client_data), "identity": l.identity }));
            rec.save_skin(&l.client_data);
        }
        println!("{} logging in", l.identity["DisplayName"]);
        self.game_key = Some(l.game_key);
        Ok(l.upstream)
    }

    /// Records a server message and returns the game messages to send: a new one after each packet
    /// that switches the codec.
    pub fn on_server_message(&mut self, msg: &[u8]) -> Result<Out, String> {
        let mut packets = Vec::new();
        self.up_codec.decode(msg, &mut packets).map_err(|e| format!("server batch: {e}"))?;
        let (mut out, mut forward) = (Out::default(), Vec::with_capacity(packets.len()));
        for buf in packets {
            let raw = RawPacket::parse(buf.clone()).map_err(|e| format!("server packet header: {e}"))?;
            match raw.id {
                NetworkSettings::ID => {
                    self.record(false, &raw);
                    let settings: NetworkSettings = raw.decode().map_err(|e| e.to_string())?;
                    self.game.start_compression(&settings).map_err(|e| e.to_string())?;
                    let alg = Algorithm::from_settings(settings.compression_algorithm).map_err(|e| e.to_string())?;
                    self.up_codec.enable_compression(alg, settings.compression_threshold.into());
                    forward.push(buf);
                    self.flush_to_game(&mut forward, &mut out.to_game);
                }
                ServerToClientHandshake::ID => {
                    let ours = self.handshake(&raw)?;
                    self.record(false, &RawPacket::parse(ours.clone()).expect("encoded by us"));
                    forward.push(ours);
                    self.flush_to_game(&mut forward, &mut out.to_game);
                    self.game_ready = true;
                }
                _ => {
                    if raw.decode::<PlayStatus>().is_ok_and(|s| s.status == PlayStatusStatus::PlayerSpawn) {
                        self.note(json!({ "event": "spawned" }));
                    }
                    self.record(false, &raw);
                    self.chain.run(false, buf, &raw, &mut forward).map_err(|e| format!("intercepted packet: {e}"))?;
                }
            }
        }
        self.flush_to_game(&mut forward, &mut out.to_game);
        self.release_held(&mut out);
        Ok(out)
    }

    /// Batches `packets` for their sides now, or holds those whose side's handshake is not done.
    pub fn inject(&mut self, packets: impl IntoIterator<Item = (Direction, Bytes)>) -> Out {
        self.held.extend(packets);
        let mut out = Out::default();
        self.release_held(&mut out);
        out
    }

    /// One batch per side for the held injections whose side is ready, after anything already in `out`.
    fn release_held(&mut self, out: &mut Out) {
        if self.held.is_empty() {
            return;
        }
        let (mut game, mut server) = (Vec::new(), Vec::new());
        for (dir, packet) in std::mem::take(&mut self.held) {
            match dir {
                Direction::ToGame if self.game_ready => game.push(packet),
                Direction::ToServer if self.server_ready => server.push(packet),
                _ => self.held.push((dir, packet)),
            }
        }
        self.flush_to_game(&mut game, &mut out.to_game);
        if !server.is_empty() {
            out.to_server.push(self.up_codec.encode(server.iter().map(|b| &b[..])));
        }
    }

    /// Our handshake for the game in place of the server's, starting encryption where the wire has it.
    fn handshake(&mut self, raw: &RawPacket) -> Result<Bytes, String> {
        if self.wire == Wire::NetherNet {
            return Ok(self.game.plaintext_handshake());
        }
        let handshake: ServerToClientHandshake = raw.decode().map_err(|e| e.to_string())?;
        let (server_key, salt) = acacia_auth::parse_server_handshake(&handshake.token).map_err(|e| e.to_string())?;
        self.up_codec.enable_encryption(derive_key(&self.key, &server_key, &salt));
        let game_key = self.game_key.as_ref().ok_or("server handshake before the game's login")?;
        Ok(self.game.start_encryption(game_key))
    }

    /// A message transport's goodbye for the server unless the game sent its own: without one BDS
    /// keeps the player and kicks the rejoin with ServerIdConflict.
    pub fn goodbye(&mut self) -> Option<Bytes> {
        if self.game_sent_disconnect {
            return None;
        }
        let mut packet = BytesMut::new();
        encode_packet(&Disconnect { reason: DisconnectFailReason::Disconnected, hide_disconnect_reason: true, content: None }, &mut packet);
        Some(self.up_codec.encode([&packet[..]]))
    }

    /// Encodes `packets` as one game message now, before a codec change applies to later ones.
    fn flush_to_game(&mut self, packets: &mut Vec<Bytes>, out: &mut Vec<Bytes>) {
        if !packets.is_empty() {
            out.push(self.game.encode(packets));
            packets.clear();
        }
    }
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
