//! Chunk data in blob-cache mode (docs/research/blob-cache.md §2): section and biome bytes are blobs,
//! either already in the client's store or delivered later by a ClientCacheMissResponse. A sub-chunk
//! entry carries only the rest (block entities), so a section is its blob followed by that rest; a
//! request-mode LevelChunk's single blob is the column's biomes.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use acacia_client::BlobStore;
use bytes::{Bytes, BytesMut};

type Pos = (i32, i32, i32);

/// Chunk data completed by a blob.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Ready {
    Section(Pos, Bytes),
    Biomes((i32, i32), Bytes),
}

#[derive(Default)]
pub(super) struct Blobs {
    /// The connection's store, for blobs held from earlier (none in trace replays).
    store: Option<Arc<dyn BlobStore>>,
    /// Data waiting on a blob; a section holds its rest, biomes hold nothing.
    waiting: HashMap<u64, Vec<Ready>>,
    /// Blobs delivered lately, newest last, for a store that keeps hashes only (idle bots).
    recent: VecDeque<(u64, Bytes)>,
    recent_cap: usize,
}

impl Blobs {
    /// Remembers the last `cap` delivered blobs itself: a hash-only store reports them held later on.
    pub fn remembering(cap: usize) -> Self {
        Self { recent_cap: cap, ..Self::default() }
    }

    pub fn set_store(&mut self, store: Arc<dyn BlobStore>) {
        self.store = Some(store);
    }

    fn held(&self, blob_id: u64) -> Option<Bytes> {
        let recent = || self.recent.iter().rev().find(|(id, _)| *id == blob_id).map(|(_, b)| b.clone());
        self.store.as_ref().and_then(|s| s.get(blob_id)).or_else(recent)
    }

    /// The section at `pos` if its blob is held; otherwise it waits for [`Blobs::delivered`] (forever
    /// when a hash-only store held it: see [`Blobs::retain_waiting`]).
    pub fn section(&mut self, pos: Pos, blob_id: u64, rest: Bytes) -> Option<Bytes> {
        match self.held(blob_id) {
            Some(blob) => Some(join(&blob, &rest)),
            None => {
                self.waiting.entry(blob_id).or_default().push(Ready::Section(pos, rest));
                None
            }
        }
    }

    /// The column's biomes if their blob is held; otherwise they wait like sections do.
    pub fn biomes(&mut self, column: (i32, i32), blob_id: u64) -> Option<Bytes> {
        let held = self.held(blob_id);
        if held.is_none() {
            self.waiting.entry(blob_id).or_default().push(Ready::Biomes(column, Bytes::new()));
        }
        held
    }

    /// Data completed by a blob the server just sent.
    pub fn delivered(&mut self, blob_id: u64, blob: &[u8]) -> Vec<Ready> {
        if self.recent_cap > 0 {
            if self.recent.len() == self.recent_cap {
                self.recent.pop_front();
            }
            self.recent.push_back((blob_id, Bytes::copy_from_slice(blob)));
        }
        let waiting = self.waiting.remove(&blob_id).unwrap_or_default();
        waiting
            .into_iter()
            .map(|w| match w {
                Ready::Section(pos, rest) => Ready::Section(pos, join(blob, &rest)),
                Ready::Biomes(column, _) => Ready::Biomes(column, Bytes::copy_from_slice(blob)),
            })
            .collect()
    }

    /// Forgets data waiting on blobs (dimension change: it belongs to the old world).
    pub fn clear(&mut self) {
        self.waiting.clear();
    }

    /// Keeps only the waiting data at positions `keep` accepts (biomes count as y 0).
    pub fn retain_waiting(&mut self, keep: impl Fn(Pos) -> bool) {
        self.waiting.retain(|_, list| {
            list.retain(|w| match w {
                Ready::Section(pos, _) => keep(*pos),
                Ready::Biomes((x, z), _) => keep((*x, 0, *z)),
            });
            !list.is_empty()
        });
    }
}

fn join(blob: &[u8], rest: &[u8]) -> Bytes {
    let mut out = BytesMut::with_capacity(blob.len() + rest.len());
    out.extend_from_slice(blob);
    out.extend_from_slice(rest);
    out.freeze()
}

#[cfg(test)]
mod tests {
    use super::*;
    use acacia_client::MemoryBlobStore;

    #[test]
    fn held_blobs_resolve_at_once_and_missing_ones_on_delivery() {
        let store = Arc::new(MemoryBlobStore::with_payloads());
        store.insert(1, &Bytes::from_static(b"held"));
        let mut blobs = Blobs::default();
        blobs.set_store(store);
        assert_eq!(blobs.section((0, 0, 0), 1, Bytes::from_static(b"+be")).as_deref(), Some(&b"held+be"[..]));
        assert_eq!(blobs.biomes((5, 6), 1).as_deref(), Some(&b"held"[..]));
        assert_eq!(blobs.section((0, 1, 0), 2, Bytes::new()), None);
        assert_eq!(blobs.section((0, 2, 0), 2, Bytes::from_static(b"!")), None);
        assert_eq!(blobs.biomes((7, 8), 2), None);
        let done = blobs.delivered(2, b"sent");
        assert_eq!(done, vec![
            Ready::Section((0, 1, 0), Bytes::from_static(b"sent")),
            Ready::Section((0, 2, 0), Bytes::from_static(b"sent!")),
            Ready::Biomes((7, 8), Bytes::from_static(b"sent")),
        ]);
        assert!(blobs.delivered(2, b"sent").is_empty());
    }

    #[test]
    fn hash_only_stores_rely_on_remembered_blobs() {
        let mut blobs = Blobs::remembering(1);
        blobs.set_store(Arc::new(MemoryBlobStore::hashes_only()));
        assert_eq!(blobs.section((0, 0, 0), 1, Bytes::new()), None);
        assert_eq!(blobs.delivered(1, b"one").len(), 1);
        assert_eq!(blobs.section((0, 1, 0), 1, Bytes::new()).as_deref(), Some(&b"one"[..]), "a later section with the same blob");
        blobs.delivered(2, b"two");
        assert_eq!(blobs.section((0, 2, 0), 1, Bytes::new()), None, "forgotten");
        blobs.retain_waiting(|(_, y, _)| y != 2);
        assert!(blobs.delivered(1, b"one").is_empty());
    }
}
