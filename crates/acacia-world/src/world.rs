//! Chunk sharing between bots on one server (azalea's model): the [`World`] holds chunks weakly,
//! each bot's [`ChunkView`] holds strong refs to the chunks in its view, and a chunk is freed when the
//! last view drops it. A `LevelChunk` whose payload matches the live shared chunk is not decoded again.
//!
//! Every connection near a chunk receives the same updates, each at its own pace, so a lagging one
//! would replay older states over newer ones. One view owns each chunk's content: only its chunk
//! data and block updates are applied, and ownership passes on when it drops the chunk.

use std::hash::{DefaultHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Weak};

use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard};
use rustc_hash::FxHashMap;

use crate::{BlockRegistry, Chunk, Dimension, Error};

pub struct SharedChunk {
    slot: RwLock<Slot>,
    /// The view whose updates apply (0: none yet).
    owner: AtomicU64,
}

struct Slot {
    chunk: Chunk,
    /// Hash of the payload the chunk was decoded from; `None` once a block update changed it.
    payload_hash: Option<u64>,
}

impl SharedChunk {
    fn new(slot: Slot) -> Arc<SharedChunk> {
        Arc::new(SharedChunk { slot: RwLock::new(slot), owner: AtomicU64::new(NO_OWNER) })
    }

    pub fn read(&self) -> MappedRwLockReadGuard<'_, Chunk> {
        RwLockReadGuard::map(self.slot.read(), |s| &s.chunk)
    }

    /// Whether `view` may change the chunk, claiming it if nobody owns it. `NO_OWNER` (calls on the
    /// [`World`] itself) always may.
    fn claim(&self, view: u64) -> bool {
        view == NO_OWNER
            || match self.owner.compare_exchange(NO_OWNER, view, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => true,
                Err(owner) => owner == view,
            }
    }

    pub(crate) fn release(&self, view: u64) {
        let _ = self.owner.compare_exchange(view, NO_OWNER, Ordering::AcqRel, Ordering::Acquire);
    }
}

const NO_OWNER: u64 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inserted {
    Decoded,
    /// Another view already provided an identical payload; decoding was skipped.
    Shared,
}

/// How block ids arrive on the wire (`StartGame.block_network_ids_are_hashes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockIds {
    /// Palette indices (Geyser).
    Runtime,
    /// FNV-1a hashes (BDS); translated to runtime ids on decode, unknown hashes become air.
    Hashed,
}

pub struct World {
    registry: Arc<BlockRegistry>,
    dim: Dimension,
    ids: BlockIds,
    chunks: RwLock<FxHashMap<(i32, i32), Weak<SharedChunk>>>,
    pub(crate) next_view: AtomicU64,

}

impl World {
    pub fn new(registry: Arc<BlockRegistry>, dimension_id: i32, ids: BlockIds) -> Arc<World> {
        let dim = Dimension::from_id(dimension_id, registry.air_id());
        Arc::new(World { registry, dim, ids, chunks: RwLock::default(), next_view: AtomicU64::new(NO_OWNER + 1) })
    }

    /// Translates a wire block id (chunk palettes, `UpdateBlock`) to a runtime id.
    pub fn runtime_id(&self, wire_id: u32) -> u32 {
        match self.ids {
            BlockIds::Runtime => wire_id,
            BlockIds::Hashed => self.registry.runtime_id_from_hash(wire_id).unwrap_or(self.dim.air),
        }
    }

    /// Inverse of [`World::runtime_id`]: the id to put in packets such as `InventoryTransaction`.
    /// Custom blocks have no hash, so they map to 0 under [`BlockIds::Hashed`].
    pub fn wire_id(&self, runtime_id: u32) -> u32 {
        match self.ids {
            BlockIds::Runtime => runtime_id,
            BlockIds::Hashed => self.registry.get(runtime_id).map_or(0, |s| s.network_hash),
        }
    }

    pub fn registry(&self) -> &Arc<BlockRegistry> {
        &self.registry
    }

    pub fn dimension(&self) -> Dimension {
        self.dim
    }

    pub fn get(&self, x: i32, z: i32) -> Option<Arc<SharedChunk>> {
        self.chunks.read().get(&(x, z))?.upgrade()
    }

    /// Chunks still held by at least one view.
    pub fn live_chunks(&self) -> usize {
        self.chunks.read().values().filter(|w| w.strong_count() > 0).count()
    }

    /// Decodes a `LevelChunk` payload, or reuses the shared chunk when the payload is identical.
    /// A different payload for a live chunk replaces its contents in place so every view sees it.
    pub fn insert_level_chunk(
        &self,
        x: i32,
        z: i32,
        sub_chunk_count: u32,
        payload: &[u8],
    ) -> Result<(Arc<SharedChunk>, Inserted), Error> {
        self.insert_level_chunk_by(NO_OWNER, x, z, sub_chunk_count, payload)
    }

    pub(crate) fn insert_level_chunk_by(
        &self,
        view: u64,
        x: i32,
        z: i32,
        sub_chunk_count: u32,
        payload: &[u8],
    ) -> Result<(Arc<SharedChunk>, Inserted), Error> {
        let hash = payload_hash(sub_chunk_count, payload);
        if let Some(c) = self.get(x, z).filter(|c| c.slot.read().payload_hash == Some(hash) || !c.claim(view)) {
            return Ok((c, Inserted::Shared));
        }
        let chunk = Chunk::decode_mapped(x, z, self.dim, sub_chunk_count, payload, &|id| self.runtime_id(id))?;
        let fresh = Slot { chunk, payload_hash: Some(hash) };
        let mut map = self.chunks.write();
        if let Some(c) = map.get(&(x, z)).and_then(Weak::upgrade) {
            if !c.claim(view) {
                return Ok((c, Inserted::Shared));
            }
            let mut slot = c.slot.write();
            if slot.payload_hash == Some(hash) {
                drop(slot);
                return Ok((c, Inserted::Shared));
            }
            *slot = fresh;
            drop(slot);
            return Ok((c, Inserted::Decoded));
        }
        // Amortized pruning of chunks every view has dropped.
        if map.len() >= 256 && map.len().is_power_of_two() {
            map.retain(|_, w| w.strong_count() > 0);
        }
        let c = SharedChunk::new(fresh);
        c.claim(view);
        map.insert((x, z), Arc::downgrade(&c));
        Ok((c, Inserted::Decoded))
    }

    /// Applies a block update (wire id) to the live chunk containing the position; returns false if
    /// unloaded.
    pub fn set_block(&self, x: i32, y: i32, z: i32, layer: u32, wire_id: u32) -> bool {
        self.set_block_by(NO_OWNER, x, y, z, layer, wire_id)
    }

    pub(crate) fn set_block_by(&self, view: u64, x: i32, y: i32, z: i32, layer: u32, wire_id: u32) -> bool {
        let Some(c) = self.get(x >> 4, z >> 4) else { return false };
        if !c.claim(view) {
            return true;
        }
        let id = self.runtime_id(wire_id);
        let mut slot = c.slot.write();
        slot.chunk.set(x, y, z, layer, id);
        slot.payload_hash = None;
        true
    }

    /// Decodes a `SubChunk` entry into the chunk at `(x, z)`, creating an empty chunk if needed.
    pub fn insert_sub_chunk(&self, x: i32, section_y: i32, z: i32, payload: &[u8]) -> Result<Arc<SharedChunk>, Error> {
        self.insert_sub_chunk_by(NO_OWNER, x, section_y, z, payload)
    }

    pub(crate) fn insert_sub_chunk_by(&self, view: u64, x: i32, section_y: i32, z: i32, payload: &[u8]) -> Result<Arc<SharedChunk>, Error> {
        let c = self.get(x, z).unwrap_or_else(|| {
            let mut map = self.chunks.write();
            let fresh = || SharedChunk::new(Slot { chunk: Chunk::empty(x, z, self.dim), payload_hash: None });
            let c = map.get(&(x, z)).and_then(Weak::upgrade).unwrap_or_else(fresh);
            map.insert((x, z), Arc::downgrade(&c));
            c
        });
        if !c.claim(view) {
            return Ok(c);
        }
        let mut slot = c.slot.write();
        slot.chunk.set_sub_chunk_mapped(section_y, payload, &|id| self.runtime_id(id))?;
        slot.payload_hash = None;
        drop(slot);
        Ok(c)
    }
}

fn payload_hash(sub_chunk_count: u32, payload: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    h.write_u32(sub_chunk_count);
    h.write(payload);
    h.finish()
}
