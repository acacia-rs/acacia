use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use acacia_physics::BlockPos;
use acacia_world::{SharedChunk, World};

use super::terrain::Blocks;

type Pinned = HashMap<(i32, i32), Option<Arc<SharedChunk>>>;

/// Blocks read straight from a shared [`World`], owning what it needs so a search can run on a
/// blocking thread while the bot keeps ticking. Chunks are pinned on first use, and each read
/// takes only that chunk's lock, briefly.
pub struct WorldBlocks {
    world: Arc<World>,
    chunks: RefCell<Pinned>,
}

impl WorldBlocks {
    pub fn new(world: Arc<World>) -> Self {
        Self { world, chunks: RefCell::default() }
    }
}

impl Blocks for WorldBlocks {
    fn ids(&self, [x, y, z]: BlockPos) -> Option<(u32, u32)> {
        let key = (x >> 4, z >> 4);
        let mut chunks = self.chunks.borrow_mut();
        let chunk = chunks.entry(key).or_insert_with(|| self.world.get(key.0, key.1)).as_ref()?;
        let chunk = chunk.read();
        Some((chunk.block(x, y, z), chunk.liquid(x, y, z)))
    }
}
