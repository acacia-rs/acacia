//! The game-level half of one proxied player, whatever the transport. Only the encryption handshake
//! is terminated, each side getting its own key. Every other packet is recorded as it came, then
//! goes through the player's interceptors (intercept.rs) and is re-batched for the other side with
//! that side's compression and encryption; with no interceptors the batches carry what came in.
//! Over NetherNet both sides still get a handshake but stay plaintext, as with BDS.

use std::net::SocketAddr;

use acacia_auth::LoginCredentials;
use acacia_proto::packets::{
    ClientToServerHandshake, Disconnect, Login, NetworkSettings, PlayStatus, PlayStatusStatus, ServerToClientHandshake, Transfer,
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

use crate::intercept::{encode, Chain, Direction};
use crate::login;
use crate::record::SessionLog;

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
    /// Host and port of a Transfer the game was just redirected away from (transfer.rs).
    pub transfer: Option<(String, u16)>,
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
    log: Option<SessionLog>,
    /// The proxy's address as the game reaches it, when transfers are followed.
    transfer_to: Option<SocketAddr>,
    chain: Chain,
    game_ready: bool,
    server_ready: bool,
    /// Injections waiting for their direction's handshake.
    held: Vec<(Direction, Bytes)>,
}

impl Relay {
    pub fn new(wire: Wire, key: SigningKey, credentials: Option<LoginCredentials>, log: Option<SessionLog>, chain: Chain) -> Self {
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
            log,
            transfer_to: None,
            chain,
            game_ready: false,
            server_ready: false,
            held: Vec::new(),
        }
    }

    /// Sends the game back to `proxy` on a Transfer instead of to the server's target (transfer.rs).
    pub fn follow_transfers(mut self, proxy: SocketAddr) -> Self {
        self.transfer_to = Some(proxy);
        self
    }

    /// Writes an event line to the capture, if recording.
    pub fn note(&self, entry: Value) {
        if let Some(log) = &self.log {
            log.write(entry);
        }
    }

    fn record(&self, from_game: bool, raw: &RawPacket) {
        if let Some(log) = &self.log {
            log.packet(from_game, raw);
        }
    }

    fn record_outcome(&self, from_game: bool, raw: &RawPacket, packet: &Bytes, out: &[Bytes]) {
        if let Some(log) = &self.log {
            log.outcome(from_game, raw, packet, out);
        }
    }

    /// The player is gone: notes it in the capture and tells the interceptors. Call once.
    pub fn close(&mut self) {
        self.note(json!({ "event": "closed" }));
        self.chain.close();
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
            let first = forward.len();
            self.chain.run(true, buf.clone(), &raw, &mut forward).map_err(|e| format!("intercepted packet: {e}"))?;
            self.record_outcome(true, &raw, &buf, &forward[first..]);
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
        let p = &l.player;
        if let Some(log) = &self.log {
            let identity = json!({ "DisplayName": p.name, "Identity": p.uuid, "XUID": p.xuid });
            log.login(raw.body.len(), l.summary);
            log.write(json!({ "event": "login", "client_data": login::trimmed(&p.client_data), "identity": identity }));
            log.save_skin(&p.client_data);
        }
        println!("{:?} logging in", p.name);
        self.chain.login(p);
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
                    let first = forward.len();
                    self.chain.run(false, buf.clone(), &raw, &mut forward).map_err(|e| format!("intercepted packet: {e}"))?;
                    self.record_outcome(false, &raw, &buf, &forward[first..]);
                    if raw.id == Transfer::ID
                        && let Some(proxy) = self.transfer_to
                    {
                        out.transfer = self.redirect(&mut forward[first..], proxy)?.or(out.transfer);
                    }
                }
            }
        }
        self.flush_to_game(&mut forward, &mut out.to_game);
        self.release_held(&mut out);
        Ok(out)
    }

    /// Points what the interceptors left of a Transfer at `proxy`; returns where it led.
    fn redirect(&self, packets: &mut [Bytes], proxy: SocketAddr) -> Result<Option<(String, u16)>, String> {
        let mut target = None;
        for packet in packets {
            let raw = RawPacket::parse(packet.clone()).map_err(|e| format!("intercepted packet: {e}"))?;
            let Ok(mut transfer) = raw.decode::<Transfer>() else { continue };
            let host = std::mem::replace(&mut transfer.server_address, proxy.ip().to_string());
            let port = std::mem::replace(&mut transfer.port, proxy.port());
            self.note(json!({ "event": "transfer", "to": format!("{host}:{port}") }));
            *packet = encode(&transfer);
            target = Some((host, port));
        }
        Ok(target)
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
        if let Some(log) = &self.log {
            game.iter().for_each(|p| log.sent(true, "inject", p));
            server.iter().for_each(|p| log.sent(false, "inject", p));
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
