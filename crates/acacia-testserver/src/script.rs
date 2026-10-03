//! A recorded server session to replay: the server's packets with their recorded pauses, each block
//! held until the client sends the reply BDS waits for. Packets that answer client requests (sub-chunks,
//! blobs) and the login handshake are generated live instead (server.rs).

use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::time::Duration;

use acacia_proto::packets::{
    ClientCacheMissResponse, ClientToServerHandshake, NetworkSettings, RequestChunkRadius, ResourcePackClientResponse,
    ServerToClientHandshake, SetLocalPlayerAsInitialized, Subchunk,
};
use acacia_proto::{codec, Packet};
use bytes::{Bytes, BytesMut};
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;

use crate::capture::Session;

/// Client packets BDS waits for before going on with the login.
const GATES: &[u32] = &[ClientToServerHandshake::ID, ResourcePackClientResponse::ID, RequestChunkRadius::ID, SetLocalPlayerAsInitialized::ID];
/// Server packets the fake server makes itself.
const GENERATED: &[u32] = &[NetworkSettings::ID, ServerToClientHandshake::ID, Subchunk::ID, ClientCacheMissResponse::ID];

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Hold until the client has sent this packet (once more than the waits on it before).
    Wait(u32),
    /// Send after this pause (from the previous step); `packet` has its header.
    Send { delay: Duration, packet: Bytes },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Script {
    pub steps: Vec<Step>,
    /// Blobs the server can send on a cache miss, by hash.
    pub blobs: HashMap<u64, Bytes>,
}

impl Script {
    /// BDS 1.26.52 (local test server) from login to ~25 s after spawn, recorded from a bot's join.
    pub fn bds_spawn() -> Self {
        Self::decode(include_bytes!("../fixtures/bds-1.26.52.script")).expect("bundled script decodes")
    }

    /// The server side of a captured session, up to `until` after its Login.
    pub fn from_session(session: &Session, until: Duration) -> Self {
        let mut script = Self::default();
        let mut mark = 0.0;
        for p in session.packets.iter().filter(|p| p.t <= until.as_secs_f64() * 1000.0) {
            if p.from_client {
                if GATES.contains(&p.id) {
                    script.steps.push(Step::Wait(p.id));
                    mark = p.t;
                }
            } else if p.id == ClientCacheMissResponse::ID {
                if let Ok(miss) = ClientCacheMissResponse::decode(&mut &p.body[..]) {
                    script.blobs.extend(miss.blobs.into_iter().map(|b| (b.hash, b.payload)));
                }
            } else if !GENERATED.contains(&p.id) {
                let mut packet = BytesMut::with_capacity(p.body.len() + 3);
                codec::write_varint(&mut packet, p.id);
                packet.extend_from_slice(&p.body);
                let delay = Duration::from_secs_f64((p.t - mark).max(0.0) / 1000.0);
                script.steps.push(Step::Send { delay, packet: packet.freeze() });
                mark = p.t;
            }
        }
        script
    }

    /// Deflated: `u8 kind ‖ ...` per step (0 = wait `u32 id`, 1 = send `u32 µs ‖ u32 len ‖ bytes`),
    /// then `u64 hash ‖ u32 len ‖ bytes` per blob (kind 2). Little endian.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for step in &self.steps {
            match step {
                Step::Wait(id) => {
                    out.push(0);
                    out.extend_from_slice(&id.to_le_bytes());
                }
                Step::Send { delay, packet } => {
                    out.push(1);
                    out.extend_from_slice(&(delay.as_micros() as u32).to_le_bytes());
                    out.extend_from_slice(&(packet.len() as u32).to_le_bytes());
                    out.extend_from_slice(packet);
                }
            }
        }
        let mut blobs: Vec<_> = self.blobs.iter().collect();
        blobs.sort_by_key(|(h, _)| **h);
        for (hash, blob) in blobs {
            out.push(2);
            out.extend_from_slice(&hash.to_le_bytes());
            out.extend_from_slice(&(blob.len() as u32).to_le_bytes());
            out.extend_from_slice(blob);
        }
        let mut enc = DeflateEncoder::new(Vec::new(), flate2::Compression::best());
        enc.write_all(&out).expect("writing to a Vec");
        enc.finish().expect("writing to a Vec")
    }

    pub fn decode(data: &[u8]) -> io::Result<Self> {
        let mut raw = Vec::new();
        DeflateDecoder::new(data).read_to_end(&mut raw)?;
        let bad = || io::Error::new(io::ErrorKind::InvalidData, "truncated script");
        let mut r = &raw[..];
        let mut take = |n: usize| -> io::Result<&[u8]> {
            let (head, rest) = r.split_at_checked(n).ok_or_else(bad)?;
            r = rest;
            Ok(head)
        };
        let u32_le = |b: &[u8]| u32::from_le_bytes(b.try_into().expect("4 bytes"));
        let mut script = Self::default();
        while let Ok(kind) = take(1) {
            match kind[0] {
                0 => script.steps.push(Step::Wait(u32_le(take(4)?))),
                1 => {
                    let delay = Duration::from_micros(u32_le(take(4)?).into());
                    let len = u32_le(take(4)?) as usize;
                    script.steps.push(Step::Send { delay, packet: Bytes::copy_from_slice(take(len)?) });
                }
                2 => {
                    let hash = u64::from_le_bytes(take(8)?.try_into().expect("8 bytes"));
                    let len = u32_le(take(4)?) as usize;
                    script.blobs.insert(hash, Bytes::copy_from_slice(take(len)?));
                }
                _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "unknown script record")),
            }
        }
        Ok(script)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_roundtrips() {
        let script = Script {
            steps: vec![Step::Wait(4), Step::Send { delay: Duration::from_millis(31), packet: Bytes::from_static(b"\x02\x00\x00\x00\x00") }],
            blobs: HashMap::from([(9, Bytes::from_static(b"blob"))]),
        };
        assert_eq!(Script::decode(&script.encode()).unwrap(), script);
    }

    #[test]
    fn bundled_script_starts_after_the_handshake() {
        let script = Script::bds_spawn();
        assert_eq!(script.steps.first(), Some(&Step::Wait(ClientToServerHandshake::ID)));
        assert!(script.steps.iter().filter(|s| matches!(s, Step::Send { .. })).count() > 100);
        assert!(!script.blobs.is_empty());
    }
}
