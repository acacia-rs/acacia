//! One client of the fake server: the login handshake, the script replay and answers to requests.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use acacia_session::batch::BatchCodec;
use acacia_session::compression::Algorithm;
use acacia_session::crypto::derive_key;
use acacia_proto::packets::{
    ClientCacheBlobStatus, ClientCacheMissResponse, Login, NetworkSettings, NetworkSettingsCompressionAlgorithm, PlayStatus,
    PlayStatusStatus, RequestNetworkSettings, ServerToClientHandshake, Subchunk, SubchunkRequest,
};
use acacia_proto::types::{Blob, HeightMapDataType, SubChunkEntryItem, SubChunkEntryItemResult};
use acacia_proto::{codec, encode_packet, Packet, RawPacket};
use bytes::{Bytes, BytesMut};
use p384::ecdsa::SigningKey;

use crate::script::{Script, Step};

/// A packet on the wire, `t` after the server answered the Login (0 before it).
#[derive(Debug, Clone)]
pub struct Received {
    pub t: Duration,
    pub packet: RawPacket,
}

pub(crate) struct Peer {
    codec: BatchCodec,
    key: SigningKey,
    script: Script,
    step: usize,
    /// When the previous step finished (or was due): sends are timed from it.
    mark: Instant,
    /// Client packets by id, and how many of them script waits have used up.
    seen: HashMap<u32, usize>,
    used: HashMap<u32, usize>,
    login_at: Option<Instant>,
    pub spawned_at: Option<Instant>,
    pub received: Vec<Received>,
    /// What the server sent, timed like `received`.
    pub sent: Vec<Received>,
    /// Batches to send.
    pub outbox: Vec<Bytes>,
}

impl Peer {
    pub fn new(script: Script, now: Instant) -> Self {
        Self {
            codec: BatchCodec::default(),
            key: SigningKey::random(&mut rand_core::OsRng),
            script,
            step: 0,
            mark: now,
            seen: HashMap::new(),
            used: HashMap::new(),
            login_at: None,
            spawned_at: None,
            received: Vec::new(),
            sent: Vec::new(),
            outbox: Vec::new(),
        }
    }

    pub fn login_at(&self) -> Option<Instant> {
        self.login_at
    }

    pub fn on_message(&mut self, now: Instant, msg: &[u8]) -> Result<(), String> {
        let mut packets = Vec::new();
        self.codec.decode(msg, &mut packets).map_err(|e| e.to_string())?;
        for buf in packets {
            let packet = RawPacket::parse(buf).map_err(|e| e.to_string())?;
            let t = self.login_at.map_or(Duration::ZERO, |at| now - at);
            *self.seen.entry(packet.id).or_default() += 1;
            self.handle(&packet)?;
            self.received.push(Received { t, packet });
        }
        self.advance(now);
        Ok(())
    }

    fn handle(&mut self, packet: &RawPacket) -> Result<(), String> {
        match packet.id {
            RequestNetworkSettings::ID => {
                // As the local test BDS is configured (zlib, threshold 1).
                self.send_now(&[encode(&NetworkSettings {
                    compression_threshold: 1,
                    compression_algorithm: NetworkSettingsCompressionAlgorithm::Deflate,
                    client_throttle: false,
                    client_throttle_threshold: 0,
                    client_throttle_scalar: 0.0,
                })]);
                self.codec.enable_compression(Algorithm::Deflate, 1);
            }
            Login::ID => {
                let mut r = &packet.body[..];
                codec::read_i32(&mut r).map_err(|e| e.to_string())?;
                let len = codec::read_varint(&mut r).map_err(|e| e.to_string())? as usize;
                let request = r.get(..len).ok_or("Login truncated")?;
                let client_key = acacia_auth::login::client_public_key(request).map_err(|e| e.to_string())?;
                let (token, salt) = acacia_auth::login::build_server_handshake(&self.key);
                self.send_now(&[encode(&ServerToClientHandshake { token })]);
                self.codec.enable_encryption(derive_key(&self.key, &client_key, &salt));
                // The client's clock starts when the handshake leaves, not before our own signing.
                let sent = Instant::now();
                self.login_at = Some(sent);
                self.mark = sent;
            }
            SubchunkRequest::ID => {
                let request: SubchunkRequest = packet.decode().map_err(|e| e.to_string())?;
                let entries = request.requests.iter().map(|o| all_air(o.x, o.y, o.z)).collect();
                self.send_now(&[encode(&Subchunk { cache_enabled: false, dimension: request.dimension, origin: request.origin, entries })]);
            }
            ClientCacheBlobStatus::ID => {
                let status: ClientCacheBlobStatus = packet.decode().map_err(|e| e.to_string())?;
                let blobs = status.missing.iter().filter_map(|h| self.script.blobs.get(h).map(|b| Blob { hash: *h, payload: b.clone() })).collect();
                self.send_now(&[encode(&ClientCacheMissResponse { blobs })]);
            }
            _ => {}
        }
        Ok(())
    }

    /// When the next scripted send is due (`None` while waiting on the client or done).
    pub fn next_due(&self) -> Option<Instant> {
        match self.script.steps.get(self.step)? {
            Step::Send { delay, .. } if self.login_at.is_some() => Some(self.mark + *delay),
            _ => None,
        }
    }

    /// Runs the script as far as the client's packets and the clock allow; due packets share a batch.
    pub fn advance(&mut self, now: Instant) {
        if self.login_at.is_none() {
            return;
        }
        let mut batch = Vec::new();
        while let Some(step) = self.script.steps.get(self.step) {
            match step {
                Step::Wait(id) => {
                    let used = self.used.entry(*id).or_default();
                    if self.seen.get(id).copied().unwrap_or(0) <= *used {
                        break;
                    }
                    *used += 1;
                    self.mark = now;
                }
                Step::Send { delay, packet } => {
                    if self.mark + *delay > now {
                        break;
                    }
                    self.mark += *delay;
                    if is_spawn(packet) {
                        self.spawned_at = Some(now);
                    }
                    batch.push(packet.clone());
                }
            }
            self.step += 1;
        }
        self.send_now(&batch);
    }

    fn send_now(&mut self, packets: &[Bytes]) {
        if packets.is_empty() {
            return;
        }
        let t = self.login_at.map_or(Duration::ZERO, |at| Instant::now() - at);
        self.sent.extend(packets.iter().filter_map(|p| RawPacket::parse(p.clone()).ok()).map(|packet| Received { t, packet }));
        self.outbox.push(self.codec.encode(packets.iter().map(|p| &p[..])));
    }
}

fn encode<T: Packet>(packet: &T) -> Bytes {
    let mut buf = BytesMut::new();
    encode_packet(packet, &mut buf);
    buf.freeze()
}

fn is_spawn(packet: &Bytes) -> bool {
    RawPacket::parse(packet.clone()).ok().and_then(|p| p.decode::<PlayStatus>().ok()).is_some_and(|s| s.status == PlayStatusStatus::PlayerSpawn)
}

fn all_air(dx: i8, dy: i8, dz: i8) -> SubChunkEntryItem {
    SubChunkEntryItem {
        dx,
        dy,
        dz,
        result: SubChunkEntryItemResult::SuccessAllAir,
        payload: None,
        heightmap_type: HeightMapDataType::NoData,
        heightmap: None,
        render_heightmap_type: HeightMapDataType::NoData,
        render_heightmap: None,
        blob_id: None,
    }
}
