//! One bot's loaded chunks: strong refs into a shared [`World`] (see the `world` module docs).

use std::sync::atomic::Ordering;
use std::sync::Arc;

use rustc_hash::FxHashMap;

use crate::{BlockAccess, Chunk, Error, Inserted, SharedChunk, World};

pub struct ChunkView {
    world: Arc<World>,
    id: u64,
    chunks: FxHashMap<(i32, i32), Arc<SharedChunk>>,
}

impl ChunkView {
    pub fn new(world: Arc<World>) -> Self {
        let id = world.next_view.fetch_add(1, Ordering::Relaxed);
        ChunkView { world, id, chunks: FxHashMap::default() }
    }

    pub fn world(&self) -> &Arc<World> {
        &self.world
    }

    pub fn insert_level_chunk(&mut self, x: i32, z: i32, sub_chunk_count: u32, payload: &[u8]) -> Result<Inserted, Error> {
        let (c, how) = self.world.insert_level_chunk_by(self.id, x, z, sub_chunk_count, payload)?;
        self.chunks.insert((x, z), c);
        Ok(how)
    }

    pub fn insert_sub_chunk(&mut self, x: i32, section_y: i32, z: i32, payload: &[u8]) -> Result<(), Error> {
        let c = self.world.insert_sub_chunk_by(self.id, x, section_y, z, payload)?;
        self.chunks.insert((x, z), c);
        Ok(())
    }

    pub fn insert_biomes(&mut self, x: i32, z: i32, payload: &[u8]) {
        let c = self.world.insert_biomes_by(self.id, x, z, payload);
        self.chunks.insert((x, z), c);
    }

    /// Applies a block update if this view owns the chunk's updates; returns false if the chunk is not
    /// in this view.
    pub fn set_block(&self, x: i32, y: i32, z: i32, layer: u32, id: u32) -> bool {
        self.chunks.contains_key(&(x >> 4, z >> 4)) && self.world.set_block_by(self.id, x, y, z, layer, id)
    }

    pub fn remove(&mut self, x: i32, z: i32) {
        if let Some(c) = self.chunks.remove(&(x, z)) {
            c.release(self.id);
        }
    }

    /// Drops chunks farther than `radius` chunks (Chebyshev) from the center, e.g. on
    /// `NetworkChunkPublisherUpdate` / `ChunkRadiusUpdate`.
    pub fn retain_within(&mut self, center_x: i32, center_z: i32, radius: i32) {
        let id = self.id;
        self.chunks.retain(|&(x, z), c| {
            let keep = (x - center_x).abs() <= radius && (z - center_z).abs() <= radius;
            if !keep {
                c.release(id);
            }
            keep
        });
    }

    pub fn chunk(&self, x: i32, z: i32) -> Option<&Arc<SharedChunk>> {
        self.chunks.get(&(x, z))
    }

    pub fn len(&self) -> usize {
        self.chunks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    fn read(&self, x: i32, y: i32, z: i32, f: impl Fn(&Chunk, i32, i32, i32) -> u32) -> u32 {
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(c) => f(&c.read(), x, y, z),
            None => self.world.dimension().air,
        }
    }
}

impl Drop for ChunkView {
    fn drop(&mut self) {
        self.chunks.values().for_each(|c| c.release(self.id));
    }
}

impl BlockAccess for ChunkView {
    fn block(&self, x: i32, y: i32, z: i32) -> u32 {
        self.read(x, y, z, Chunk::block)
    }

    fn liquid(&self, x: i32, y: i32, z: i32) -> u32 {
        self.read(x, y, z, Chunk::liquid)
    }
}
