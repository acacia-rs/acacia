//! Server packets the session reacts to, and the replies it sends.

use acacia_proto::packets::{
    ClientCacheStatus, ClientCameraAimAssist, ClientCameraAimAssistAction, ClientToServerHandshake, Disconnect, ItemRegistry, Login, NetworkSettings, NetworkStackLatency,
    PacketViolationWarning,
    PlayStatus, PlayStatusStatus, PlayerAction, RequestChunkRadius,
    ResourcePackChunkData, ResourcePackDataInfo, ResourcePackStack, ResourcePacksInfo,
    Respawn, ServerToClientHandshake, SetLocalPlayerAsInitialized, StartGame, Transfer,
};
use acacia_proto::types::{Action, BlockCoordinates, Vec3f};
use acacia_proto::{codec, Packet, RawPacket, PROTOCOL_VERSION};
use bytes::BytesMut;

use super::blob_cache::BlobStatus;
use super::deferred::delay;
use super::{DisconnectReason, Event, Session, Stage};
use crate::compression::Algorithm;
use crate::crypto::derive_key;
use crate::Error;

/// Respawn states: the server is searching for a spawn point / has one / the client is ready.
const RESPAWN_SEARCHING: u8 = 0;
const RESPAWN_SERVER_READY: u8 = 1;
const RESPAWN_CLIENT_READY: u8 = 2;

/// Vanilla non-PlayStation clients echo NetworkStackLatency timestamps multiplied by this (PS5 uses
/// 1000); Boar divides replies by it to match its probe IDs.
const LATENCY_MAGNITUDE: u64 = 1_000_000;

impl Session {
    pub(super) fn handle_packet(&mut self, raw: RawPacket) -> Result<Option<DisconnectReason>, Error> {
        tracing::trace!(id = raw.id, len = raw.body.len(), stage = ?self.stage, "packet");
        self.audit(&raw);
        if let Some(blobs) = &mut self.blobs
            && BlobStatus::PACKETS.contains(&raw.id)
            && let Err(e) = blobs.apply(self.now, &raw)
        {
            tracing::warn!(id = raw.id, error = %e, "chunk packet unreadable for the blob cache");
        }
        match raw.id {
            NetworkSettings::ID => {
                let settings: NetworkSettings = raw.decode()?;
                let alg = Algorithm::from_settings(settings.compression_algorithm)?;
                self.flush();
                self.codec.enable_compression(alg, settings.compression_threshold.into());
                self.send_login();
            }
            ServerToClientHandshake::ID => {
                let handshake: ServerToClientHandshake = raw.decode()?;
                let (server_key, salt) = acacia_auth::parse_server_handshake(&handshake.token)
                    .map_err(|e| Error::Handshake(e.to_string()))?;
                self.flush();
                // BDS still sends the handshake over NetherNet but expects plaintext; DTLS encrypts.
                if !self.link.is_message() {
                    self.codec.enable_encryption(derive_key(&self.key, &server_key, &salt));
                }
                self.send_later(delay::HANDSHAKE, &ClientToServerHandshake {});
            }
            PlayStatus::ID => match raw.decode::<PlayStatus>()?.status {
                PlayStatusStatus::LoginSuccess => {
                    let enabled = self.blobs.is_some();
                    self.send_later(delay::CACHE_STATUS, &ClientCacheStatus { enabled });
                }
                PlayStatusStatus::PlayerSpawn => self.on_spawn(),
                failed => return Ok(Some(DisconnectReason::LoginFailed(failed))),
            },
            ResourcePacksInfo::ID => self.on_packs_info(&raw)?,
            ResourcePackDataInfo::ID => self.on_pack_data_info(&raw)?,
            ResourcePackChunkData::ID => self.on_pack_chunk(&raw)?,
            ResourcePackStack::ID => self.on_pack_stack(),
            StartGame::ID => self.runtime_entity_id = Some(read_runtime_entity_id(&raw.body)?),
            ItemRegistry::ID => {
                if let Some(shield) = acacia_proto::manual::shield_item_id_in_registry(&raw.body)? {
                    acacia_proto::manual::set_shield_item_id(shield);
                }
                // Vanilla asks for its view distance after the item registry, with its device maximum.
                let max_radius = acacia_auth::MAX_VIEW_DISTANCE as u8;
                self.send_later(delay::CHUNK_RADIUS, &RequestChunkRadius { chunk_radius: self.chunk_radius, max_radius });
            }
            // Anticheats (e.g. Boar: max-latency-wait 15s) kick clients that leave these unanswered.
            NetworkStackLatency::ID => {
                let ping: NetworkStackLatency = raw.decode()?;
                if ping.needs_response != 0 {
                    let timestamp = ping.timestamp.wrapping_mul(LATENCY_MAGNITUDE);
                    self.send(&NetworkStackLatency { timestamp, needs_response: 0 });
                }
            }
            // Joining, BDS sends "searching" twice and then "ready" without a death; vanilla answers no
            // "searching" before that first "ready", so only a later one is a death screen.
            Respawn::ID => match raw.decode::<Respawn>()?.state {
                RESPAWN_SERVER_READY => {
                    self.respawn_ready_seen = true;
                    if std::mem::take(&mut self.respawn_requested) {
                        self.send_respawn_done();
                    }
                }
                RESPAWN_SEARCHING if self.auto_respawn && self.respawn_ready_seen => self.respawn(),
                _ => {}
            },
            Transfer::ID => {
                let t: Transfer = raw.decode()?;
                return Ok(Some(DisconnectReason::Transfer { address: t.server_address, port: t.port }));
            }
            // BDS answers a packet it cannot parse with this and then drops the connection without a Disconnect.
            PacketViolationWarning::ID => {
                if let Ok(w) = raw.decode::<PacketViolationWarning>() {
                    tracing::warn!(packet = w.packet_id, kind = ?w.violation_type, severity = ?w.severity, context = %w.reason, "server rejected a packet");
                }
            }
            Disconnect::ID => {
                let d: Disconnect = raw.decode()?;
                let message = d.content.map(|c| c.message).unwrap_or_default();
                return Ok(Some(DisconnectReason::Kicked { reason: format!("{:?}", d.reason), message }));
            }
            _ => {}
        }
        self.events.push_back(Event::Packet(raw));
        Ok(None)
    }

    /// Presses "respawn" on the death screen, as the vanilla client does: `Respawn(client ready)`
    /// now, the rest once the server answers (`send_respawn_done`). No-op before StartGame.
    pub fn respawn(&mut self) {
        let Some(runtime_entity_id) = self.runtime_entity_id else { return };
        self.respawn_requested = true;
        self.send(&Respawn { position: Vec3f { x: 0.0, y: 0.0, z: 0.0 }, state: RESPAWN_CLIENT_READY, runtime_entity_id });
    }

    /// What vanilla sends when it is back in the world, after a respawn and after spawning:
    /// `PlayerAction(Respawn)` and a cleared camera aim assist.
    pub fn send_respawn_done(&mut self) {
        let Some(runtime_entity_id) = self.runtime_entity_id else { return };
        let origin = BlockCoordinates { x: 0, y: 0, z: 0 };
        self.send(&PlayerAction { runtime_entity_id, action: Action::Respawn, position: origin.clone(), result_position: origin, face: -1 });
        self.send(&ClientCameraAimAssist { preset_id: String::new(), action: ClientCameraAimAssistAction::Clear, allow_aim_assist: false });
    }

    /// Written by hand: `Login.tokens` is an opaque length-prefixed blob that acacia-auth builds whole.
    fn send_login(&mut self) {
        let tokens = std::mem::take(&mut self.login_request);
        let mut buf = BytesMut::with_capacity(tokens.len() + 16);
        codec::write_varint(&mut buf, Login::ID);
        codec::write_i32(&mut buf, PROTOCOL_VERSION);
        codec::write_varint(&mut buf, tokens.len() as u32);
        buf.extend_from_slice(&tokens);
        self.deferred.push(self.now, delay::LOGIN, buf.freeze());
    }

    fn on_spawn(&mut self) {
        let Some(runtime_entity_id) = self.runtime_entity_id else {
            tracing::warn!("PlayerSpawn before StartGame; ignoring");
            return;
        };
        tracing::debug!(runtime_entity_id, "spawned");
        if self.initialize_on_spawn {
            self.send(&SetLocalPlayerAsInitialized { runtime_entity_id });
        }
        self.stage = Stage::Spawned;
        if let Some(blobs) = &mut self.blobs {
            blobs.spawned(self.now);
        }
        self.events.push_back(Event::Spawned { runtime_entity_id });
    }
}

/// StartGame is huge; the session only needs its second field, so skip a full decode.
fn read_runtime_entity_id(body: &[u8]) -> Result<u64, Error> {
    let mut r = body;
    codec::read_zigzag64(&mut r)?;
    Ok(codec::read_varint64(&mut r)?)
}
