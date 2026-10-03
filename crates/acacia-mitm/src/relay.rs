//! The game-level half of one proxied player, whatever the transport. Only the encryption handshake
//! is terminated, each side getting its own key; every other packet passes through unchanged,
//! batched as it came, so the game's own replies (packs, cache status, chunk radius) are what the
//! server sees. Over NetherNet both sides still get a handshake but stay plaintext, as with BDS.

use acacia_auth::LoginCredentials;
use acacia_proto::packets::{Disconnect, Login, NetworkSettings, PlayStatus, PlayStatusStatus, ServerToClientHandshake};
use acacia_proto::types::DisconnectFailReason;
use acacia_proto::{encode_packet, Packet, RawPacket};
use acacia_session::batch::BatchCodec;
use acacia_session::compression::Algorithm;
use acacia_session::crypto::derive_key;
use acacia_session::server::ServerConnection;
use bytes::{Bytes, BytesMut};
use p384::ecdsa::SigningKey;
use serde_json::json;

use crate::login;
use crate::record::Recorder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wire {
    RakNet,
    NetherNet,
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
}

impl Relay {
    pub fn new(wire: Wire, key: SigningKey, credentials: Option<LoginCredentials>) -> Self {
        let (game, up_codec) = match wire {
            Wire::RakNet => (ServerConnection::new(key.clone()), BatchCodec::default()),
            Wire::NetherNet => (ServerConnection::nethernet(key.clone()), BatchCodec::without_header()),
        };
        Self { wire, game, up_codec, key, credentials, game_key: None, game_sent_disconnect: false }
    }

    /// Records a game message and returns it re-batched for the server, Login re-signed.
    pub fn on_game_message(&mut self, msg: &[u8], rec: &mut Recorder) -> Result<Bytes, String> {
        let packets = self.game.decode(msg).map_err(|e| format!("game batch: {e}"))?;
        let mut forward = Vec::with_capacity(packets.len());
        for buf in packets {
            let raw = RawPacket::parse(buf.clone()).map_err(|e| format!("game packet header: {e}"))?;
            if raw.id != Login::ID {
                self.game_sent_disconnect |= raw.id == Disconnect::ID;
                rec.packet(true, &raw);
                forward.push(buf);
                continue;
            }
            let l = login::read(&raw.body, &self.key, self.credentials.as_ref()).map_err(|e| format!("game login: {e}"))?;
            rec.login(raw.body.len(), l.summary);
            rec.write(json!({ "event": "login", "client_data": login::trimmed(&l.client_data), "identity": l.identity }));
            rec.save_skin(&l.client_data);
            println!("{} logging in", l.identity["DisplayName"]);
            self.game_key = Some(l.game_key);
            forward.push(l.upstream);
        }
        Ok(self.up_codec.encode(forward.iter().map(|b| &b[..])))
    }

    /// Records a server message and returns the game messages to send: a new one after each packet
    /// that switches the codec.
    pub fn on_server_message(&mut self, msg: &[u8], rec: &mut Recorder) -> Result<Vec<Bytes>, String> {
        let mut packets = Vec::new();
        self.up_codec.decode(msg, &mut packets).map_err(|e| format!("server batch: {e}"))?;
        let (mut out, mut forward) = (Vec::new(), Vec::with_capacity(packets.len()));
        for buf in packets {
            let raw = RawPacket::parse(buf.clone()).map_err(|e| format!("server packet header: {e}"))?;
            match raw.id {
                NetworkSettings::ID => {
                    rec.packet(false, &raw);
                    let settings: NetworkSettings = raw.decode().map_err(|e| e.to_string())?;
                    self.game.start_compression(&settings).map_err(|e| e.to_string())?;
                    let alg = Algorithm::from_settings(settings.compression_algorithm).map_err(|e| e.to_string())?;
                    self.up_codec.enable_compression(alg, settings.compression_threshold.into());
                    forward.push(buf);
                    self.flush_to_game(&mut forward, &mut out);
                }
                ServerToClientHandshake::ID => {
                    let ours = self.handshake(&raw)?;
                    rec.packet(false, &RawPacket::parse(ours.clone()).expect("encoded by us"));
                    forward.push(ours);
                    self.flush_to_game(&mut forward, &mut out);
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
        self.flush_to_game(&mut forward, &mut out);
        Ok(out)
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
