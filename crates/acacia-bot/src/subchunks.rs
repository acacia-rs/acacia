//! Vanilla's bulk `SubchunkRequest` stream for bots that don't keep terrain (idle mode); physics bots
//! request through `world`. Every column within dx²+dz² ≤ 17 of the player's chunk, sub-chunks from the
//! dimension floor up to the LevelChunk's limit, in shuffled column order: at first up to 3 columns
//! per packet with ~6 packets in flight, then one column per packet once nothing is outstanding
//! (docs/research/subchunk-requests.md; its camera-level stream is not modelled).

use std::collections::{HashMap, HashSet};

use acacia_client::proto::packets::{LevelChunk, Subchunk, SubchunkRequest};
use acacia_client::proto::types::{Vec3i8, Vec3li};
use acacia_client::proto::{Packet, RawPacket};
use acacia_world::Dimension;

const RADIUS_SQ: i32 = 17;
const STARTUP_COLUMNS_PER_PACKET: usize = 3;
const STARTUP_IN_FLIGHT: u32 = 6;

pub(crate) struct SubChunkRequester {
    /// Sub-chunk count per column awaiting a request.
    limits: HashMap<(i32, i32), u8>,
    requested: HashSet<(i32, i32)>,
    dimension: i32,
    in_flight: u32,
    /// Ticks to wait after a response before the next steady-state request.
    pause: u32,
    startup: bool,
    rng: u64,
}

impl Default for SubChunkRequester {
    fn default() -> Self {
        Self::new(crate::cadence::seed())
    }
}

impl SubChunkRequester {
    pub const PACKETS: &[u32] = &[LevelChunk::ID, Subchunk::ID];

    pub fn new(seed: u64) -> Self {
        Self { limits: HashMap::new(), requested: HashSet::new(), dimension: 0, in_flight: 0, pause: 0, startup: true, rng: seed }
    }

    pub fn apply(&mut self, packet: &RawPacket) {
        if let Ok(c) = packet.decode::<LevelChunk>() {
            // Request mode: a sub-chunk limit (negative = the whole height) instead of inline sub-chunks.
            let Some(limit) = c.highest_subchunk_count else { return };
            if c.dimension != self.dimension {
                (self.dimension, self.startup) = (c.dimension, true);
                self.limits.clear();
                self.requested.clear();
            }
            let full = (Dimension::from_id(c.dimension, 0).height >> 4) as i32;
            let count = if limit < 0 { full } else { limit.min(full) };
            if !self.requested.contains(&(c.x, c.z)) {
                self.limits.insert((c.x, c.z), count as u8);
            }
        } else if packet.id == Subchunk::ID {
            self.in_flight = self.in_flight.saturating_sub(1);
            self.pause = (crate::cadence::splitmix(&mut self.rng) % 2) as u32;
        }
    }

    /// Requests to send this tick for a player whose feet are at block `(x, z)`.
    pub fn tick(&mut self, x: f32, z: f32) -> Vec<SubchunkRequest> {
        let (cx, cz) = ((x.floor() as i32) >> 4, (z.floor() as i32) >> 4);
        let mut due: Vec<(i32, i32)> = self
            .limits
            .keys()
            .copied()
            .filter(|&(x, z)| (x - cx).pow(2) + (z - cz).pow(2) <= RADIUS_SQ)
            .collect();
        if due.is_empty() {
            self.startup = self.startup && self.requested.is_empty();
            return Vec::new();
        }
        due.sort_unstable();
        for i in (1..due.len()).rev() {
            due.swap(i, (crate::cadence::splitmix(&mut self.rng) % (i as u64 + 1)) as usize);
        }
        let mut out = Vec::new();
        if self.startup {
            for columns in due.chunks(STARTUP_COLUMNS_PER_PACKET) {
                if self.in_flight >= STARTUP_IN_FLIGHT {
                    break;
                }
                out.push(self.request(cx, cz, columns));
            }
        } else if self.in_flight == 0 {
            if self.pause > 0 {
                self.pause -= 1;
            } else {
                out.push(self.request(cx, cz, &due[..1]));
            }
        }
        out
    }

    fn request(&mut self, cx: i32, cz: i32, columns: &[(i32, i32)]) -> SubchunkRequest {
        let floor = Dimension::from_id(self.dimension, 0).min_y >> 4;
        let mut requests = Vec::new();
        for &(x, z) in columns {
            let count = self.limits.remove(&(x, z)).unwrap_or(0) as i32;
            self.requested.insert((x, z));
            requests.extend((floor..floor + count).map(|y| Vec3i8 { x: (x - cx) as i8, y: y as i8, z: (z - cz) as i8 }));
        }
        self.in_flight += 1;
        SubchunkRequest { dimension: self.dimension, requests, origin: Vec3li { x: cx, y: 0, z: cz } }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acacia_client::proto::encode_packet;
    use bytes::BytesMut;

    fn raw<T: Packet>(p: &T) -> RawPacket {
        let mut buf = BytesMut::new();
        encode_packet(p, &mut buf);
        RawPacket::parse(buf.freeze()).unwrap()
    }

    fn chunk(x: i32, z: i32, limit: i32) -> RawPacket {
        raw(&LevelChunk { x, z, dimension: 0, sub_chunk_count: 0, highest_subchunk_count: Some(limit), cache_enabled: false, blobs: Vec::new(), payload: Default::default() })
    }

    fn response() -> RawPacket {
        raw(&Subchunk { cache_enabled: false, dimension: 0, origin: Vec3li { x: 0, y: 0, z: 0 }, entries: Vec::new() })
    }

    #[test]
    fn bulk_circle_then_one_column_at_a_time() {
        let mut r = SubChunkRequester::new(1);
        for x in -5..=5 {
            for z in -5..=5 {
                r.apply(&chunk(x, z, 10));
            }
        }
        // Player in chunk (1, 0).
        let first = r.tick(20.0, 3.0);
        assert_eq!(first.len(), STARTUP_IN_FLIGHT as usize);
        for req in &first {
            assert_eq!((req.origin.x, req.origin.y, req.origin.z), (1, 0, 0));
            assert!(req.requests.len() <= 3 * 10 && req.requests.len() % 10 == 0);
            assert!(req.requests.iter().all(|o| (-4..6).contains(&o.y) && i32::from(o.x).pow(2) + i32::from(o.z).pow(2) <= RADIUS_SQ));
        }
        assert!(r.tick(20.0, 3.0).is_empty(), "waits for responses at the in-flight cap");
        let mut columns: HashSet<(i8, i8)> = first.iter().flat_map(|q| q.requests.iter().map(|o| (o.x, o.z))).collect();
        let circle = (-5..=5).flat_map(|x| (-5..=5).map(move |z| (x, z))).filter(|(x, z): &(i32, i32)| x * x + z * z <= RADIUS_SQ).count();
        for _ in 0..200 {
            r.apply(&response());
            let sent = r.tick(20.0, 3.0);
            assert!(sent.len() <= STARTUP_IN_FLIGHT as usize);
            columns.extend(sent.iter().flat_map(|q| q.requests.iter().map(|o| (o.x, o.z))));
        }
        assert_eq!(columns.len(), circle, "every column in the circle, once");
        assert!(r.tick(20.0, 3.0).is_empty());

        // Walking one chunk east: only the new edge, one column per packet.
        let mut later = Vec::new();
        for _ in 0..50 {
            later.extend(r.tick(36.0, 3.0));
            r.apply(&response());
        }
        assert!(!later.is_empty() && later.iter().all(|q| q.requests.len() == 10 && q.origin.x == 2));
        let newly_covered = |q: &SubchunkRequest| {
            let (x, z) = (i32::from(q.requests[0].x) + 2, i32::from(q.requests[0].z));
            (x - 1).pow(2) + z * z > RADIUS_SQ
        };
        assert!(later.iter().all(newly_covered), "old columns are never re-requested");
    }
}
