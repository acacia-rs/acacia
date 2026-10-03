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

pub use acacia_proto::xxh64;

#[cfg(test)]
mod tests {
    use super::*;

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
