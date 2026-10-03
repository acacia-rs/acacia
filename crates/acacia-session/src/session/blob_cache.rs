//! What the client tells the server about chunk blobs (docs/research/blob-cache.md §3-5): nothing until
//! spawn, then one ClientCacheBlobStatus per client tick covering every hash received since the last.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

use acacia_proto::packets::{ClientCacheBlobStatus, ClientCacheMissResponse, LevelChunk, Subchunk};
use acacia_proto::{DecodeError, Packet, RawPacket};

use super::deferred::{self, delay};
use crate::blob_store::{xxh64, BlobStore};

pub(super) struct BlobStatus {
    store: Arc<dyn BlobStore>,
    /// Hash slots since the last status, in arrival order; a hash used twice is listed twice.
    slots: Vec<u64>,
    /// Reported missing and not yet answered: listed as held so they are not asked for twice.
    in_flight: HashSet<u64>,
    spawned: bool,
    due: Option<Instant>,
    rng: u64,
}

impl BlobStatus {
    pub const PACKETS: &[u32] = &[LevelChunk::ID, Subchunk::ID, ClientCacheMissResponse::ID];

    pub fn new(store: Arc<dyn BlobStore>) -> Self {
        Self { store, slots: Vec::new(), in_flight: HashSet::new(), spawned: false, due: None, rng: deferred::seed() }
    }

    pub fn apply(&mut self, now: Instant, raw: &RawPacket) -> Result<(), DecodeError> {
        match raw.id {
            LevelChunk::ID => {
                let chunk: LevelChunk = raw.decode()?;
                if chunk.cache_enabled {
                    self.slots.extend(chunk.blobs);
                }
            }
            Subchunk::ID => {
                let sub: Subchunk = raw.decode()?;
                if sub.cache_enabled {
                    self.slots.extend(sub.entries.iter().filter_map(|e| e.blob_id));
                }
            }
            ClientCacheMissResponse::ID => {
                for blob in raw.decode::<ClientCacheMissResponse>()?.blobs {
                    self.in_flight.remove(&blob.hash);
                    if xxh64(&blob.payload) == blob.hash {
                        self.store.insert(blob.hash, &blob.payload);
                    } else {
                        tracing::warn!(hash = blob.hash, "blob does not match its hash; dropped");
                    }
                }
            }
            _ => {}
        }
        if self.spawned && self.due.is_none() && !self.slots.is_empty() {
            self.due = Some(now + deferred::wait(&mut self.rng, delay::BLOB_STATUS));
        }
        Ok(())
    }

    pub fn spawned(&mut self, now: Instant) {
        self.spawned = true;
        self.due = Some(now + deferred::wait(&mut self.rng, delay::FIRST_BLOB_STATUS));
    }

    pub fn next_due(&self) -> Option<Instant> {
        self.due
    }

    pub fn poll(&mut self, now: Instant) -> Option<ClientCacheBlobStatus> {
        if self.due? > now {
            return None;
        }
        self.due = None;
        if self.slots.is_empty() {
            return None;
        }
        let mut status = ClientCacheBlobStatus { missing: Vec::new(), have: Vec::new() };
        for hash in self.slots.drain(..) {
            if self.store.has(hash) || self.in_flight.contains(&hash) {
                status.have.push(hash);
            } else {
                self.in_flight.insert(hash);
                status.missing.push(hash);
            }
        }
        Some(status)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use acacia_proto::encode_packet;
    use acacia_proto::types::{Blob, Vec3li};
    use bytes::{Bytes, BytesMut};

    use super::*;
    use crate::blob_store::MemoryBlobStore;

    fn raw<T: Packet>(p: &T) -> RawPacket {
        let mut buf = BytesMut::new();
        encode_packet(p, &mut buf);
        RawPacket::parse(buf.freeze()).unwrap()
    }

    fn chunk(blobs: Vec<u64>) -> RawPacket {
        raw(&LevelChunk { x: 0, z: 0, dimension: 0, sub_chunk_count: 0, highest_subchunk_count: Some(4), cache_enabled: true, blobs, payload: Bytes::from_static(&[0]) })
    }

    #[test]
    fn silent_until_spawn_then_lists_every_slot_once() {
        let store = Arc::new(MemoryBlobStore::hashes_only());
        let mut s = BlobStatus::new(store.clone());
        let t0 = Instant::now();
        let section = Bytes::from_static(b"section bytes");
        let id = xxh64(&section);
        s.apply(t0, &chunk(vec![id, 5, id])).unwrap();
        assert_eq!(s.next_due(), None, "no status before spawn");

        s.spawned(t0);
        let first = s.next_due().unwrap() - t0;
        assert!((380..=450).contains(&(first.as_millis() as u64)));
        let status = s.poll(t0 + Duration::from_secs(1)).unwrap();
        assert_eq!((status.missing, status.have), (vec![id, 5], vec![id]), "missing deduped, repeats held");

        s.apply(t0, &raw(&ClientCacheMissResponse { blobs: vec![Blob { hash: id, payload: section }, Blob { hash: 5, payload: Bytes::from_static(b"forged") }] })).unwrap();
        assert!(store.has(id) && !store.has(5), "only blobs matching their hash are kept");

        let t1 = t0 + Duration::from_secs(2);
        s.apply(t1, &raw(&Subchunk { cache_enabled: true, dimension: 0, origin: Vec3li { x: 0, y: 0, z: 0 }, entries: Vec::new() })).unwrap();
        assert_eq!(s.next_due(), None, "nothing to report");
        s.apply(t1, &chunk(vec![id])).unwrap();
        let due = s.next_due().unwrap() - t1;
        assert!((40..=130).contains(&(due.as_millis() as u64)));
        let status = s.poll(t1 + Duration::from_secs(1)).unwrap();
        assert_eq!((status.missing, status.have), (vec![], vec![id]));
    }
}
