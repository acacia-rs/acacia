//! The client blob cache: chunk sections and biome data the server sends once and then refers to by
//! hash (docs/research/blob-cache.md). The session reports which hashes it has; a [`BlobStore`] decides
//! what is kept, in memory or on disk (`acacia-client`).

use std::collections::HashMap;
use std::sync::Mutex;

use bytes::Bytes;

/// Blobs by id (the xxHash64 of their bytes, see [`xxh64`]). Shared by the session, which reports and
/// fills it, and a terrain tracker, which reads payloads.
pub trait BlobStore: Send + Sync {
    /// Whether to report `id` as held. Stores without payloads may only claim blobs the caller never
    /// needs to read (idle bots never decode terrain).
    fn has(&self, id: u64) -> bool;
    /// The blob's bytes; `None` if unknown or if this store keeps hashes only.
    fn get(&self, id: u64) -> Option<Bytes>;
    /// A blob the server sent, already checked against its id.
    fn insert(&self, id: u64, blob: &Bytes);
    fn keeps_payloads(&self) -> bool;
}

/// A store that lives as long as the connection.
pub struct MemoryBlobStore {
    payloads: bool,
    blobs: Mutex<HashMap<u64, Option<Bytes>>>,
}

impl MemoryBlobStore {
    pub fn hashes_only() -> Self {
        Self { payloads: false, blobs: Mutex::default() }
    }

    pub fn with_payloads() -> Self {
        Self { payloads: true, blobs: Mutex::default() }
    }
}

impl BlobStore for MemoryBlobStore {
    fn has(&self, id: u64) -> bool {
        self.blobs.lock().expect("blob store lock").contains_key(&id)
    }

    fn get(&self, id: u64) -> Option<Bytes> {
        self.blobs.lock().expect("blob store lock").get(&id).cloned().flatten()
    }

    fn insert(&self, id: u64, blob: &Bytes) {
        let kept = self.payloads.then(|| blob.clone());
        self.blobs.lock().expect("blob store lock").insert(id, kept);
    }

    fn keeps_payloads(&self) -> bool {
        self.payloads
    }
}

const P1: u64 = 0x9e37_79b1_85eb_ca87;
const P2: u64 = 0xc2b2_ae3d_27d4_eb4f;
const P3: u64 = 0x1656_67b1_9e37_79f9;
const P4: u64 = 0x85eb_ca77_c2b2_ae63;
const P5: u64 = 0x27d4_eb2f_1656_67c5;

/// xxHash64 with seed 0: how the server names blobs.
pub fn xxh64(data: &[u8]) -> u64 {
    let round = |acc: u64, lane: u64| acc.wrapping_add(lane.wrapping_mul(P2)).rotate_left(31).wrapping_mul(P1);
    let u64_at = |b: &[u8]| u64::from_le_bytes(b[..8].try_into().expect("8 bytes"));
    let mut rest = data;
    let mut h = if data.len() >= 32 {
        let mut v = [P1.wrapping_add(P2), P2, 0, 0u64.wrapping_sub(P1)];
        while rest.len() >= 32 {
            for (i, lane) in v.iter_mut().enumerate() {
                *lane = round(*lane, u64_at(&rest[i * 8..]));
            }
            rest = &rest[32..];
        }
        let mut h = v[0].rotate_left(1).wrapping_add(v[1].rotate_left(7)).wrapping_add(v[2].rotate_left(12)).wrapping_add(v[3].rotate_left(18));
        for lane in v {
            h = (h ^ round(0, lane)).wrapping_mul(P1).wrapping_add(P4);
        }
        h
    } else {
        P5
    };
    h = h.wrapping_add(data.len() as u64);
    while rest.len() >= 8 {
        h = (h ^ round(0, u64_at(rest))).rotate_left(27).wrapping_mul(P1).wrapping_add(P4);
        rest = &rest[8..];
    }
    if rest.len() >= 4 {
        let lane = u64::from(u32::from_le_bytes(rest[..4].try_into().expect("4 bytes")));
        h = (h ^ lane.wrapping_mul(P1)).rotate_left(23).wrapping_mul(P2).wrapping_add(P3);
        rest = &rest[4..];
    }
    for &b in rest {
        h = (h ^ u64::from(b).wrapping_mul(P5)).rotate_left(11).wrapping_mul(P1);
    }
    h ^= h >> 33;
    h = h.wrapping_mul(P2);
    h ^= h >> 29;
    h = h.wrapping_mul(P3);
    h ^ (h >> 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xxh64_reference_vectors() {
        assert_eq!(xxh64(b""), 0xef46_db37_51d8_e999);
        assert_eq!(xxh64(b"abc"), 0x44bc_2cf5_ad77_0999);
        assert_eq!(xxh64(b"Nobody inspects the spammish repetition"), 0xfbce_a83c_8a37_8bf1);
    }

    #[test]
    fn hash_only_store_claims_without_payload() {
        let (lean, full) = (MemoryBlobStore::hashes_only(), MemoryBlobStore::with_payloads());
        let blob = Bytes::from_static(b"section");
        for store in [&lean, &full] {
            store.insert(7, &blob);
            assert!(store.has(7) && !store.has(8));
        }
        assert_eq!((lean.get(7), full.get(7)), (None, Some(blob)));
    }
}
