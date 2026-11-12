//! Loaded chunk columns by state id. Columns are shared, so a clone is a cheap snapshot for work off the tick.

use rustc_hash::FxHashMap;
use std::sync::Arc;

use acacia_java_proto::chunk::Section;
use acacia_registry::BlockState;

pub(crate) fn is_air(state: u32) -> bool {
    BlockState::from_id(state).is_some_and(BlockState::is_air)
}

#[derive(Debug, Clone, Default)]
pub struct Chunks {
    /// Fx, not SipHash: a block read hashes its column key, and simulated movement reads thousands a tick.
    columns: FxHashMap<(i32, i32), Arc<Vec<Section>>>,
    min_y: i32,
}

impl Chunks {
    pub(crate) fn new(min_y: i32) -> Self {
        Self { columns: FxHashMap::default(), min_y }
    }

    pub fn len(&self) -> usize {
        self.columns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn min_y(&self) -> i32 {
        self.min_y
    }

    /// The state id at a position, `None` outside the loaded sections.
    pub fn block(&self, [x, y, z]: [i32; 3]) -> Option<u32> {
        let (section, index) = self.locate([x, y, z])?;
        self.columns.get(&(x >> 4, z >> 4))?.get(section).map(|s| s.blocks.get(index))
    }

    fn locate(&self, [x, y, z]: [i32; 3]) -> Option<(usize, usize)> {
        let section = usize::try_from((y - self.min_y) >> 4).ok()?;
        Some((section, Section::index((x & 15) as usize, (y & 15) as usize, (z & 15) as usize)))
    }

    /// `ChunkAccess.getNoiseBiome`: the biome id of a 4×4×4 cell, y clamped into the column; `None` unloaded.
    pub fn noise_biome(&self, [qx, qy, qz]: [i32; 3]) -> Option<u32> {
        let column = self.columns.get(&(qx >> 2, qz >> 2))?;
        let low = self.min_y >> 2;
        let qy = qy.clamp(low, low + column.len() as i32 * 4 - 1);
        let section = column.get(((qy - low) >> 2) as usize)?;
        Some(section.biomes.get((((qy & 3) << 4) | ((qz & 3) << 2) | (qx & 3)) as usize))
    }

    /// The top of the build height, exclusive.
    pub fn max_y(&self, x: i32, z: i32) -> Option<i32> {
        self.columns.get(&(x >> 4, z >> 4)).map(|c| self.min_y + c.len() as i32 * 16)
    }

    /// Copies the column first when a snapshot still holds it. Returns the old state; keeps the non-air count.
    pub(crate) fn set_block(&mut self, pos: [i32; 3], state: u32) -> Option<u32> {
        let (section, index) = self.locate(pos)?;
        let s = self.columns.get_mut(&(pos[0] >> 4, pos[2] >> 4)).and_then(|c| Arc::make_mut(c).get_mut(section))?;
        let old = s.blocks.get(index);
        s.set_block(index, state);
        s.non_empty += i16::from(is_air(old)) - i16::from(is_air(state));
        Some(old)
    }

    /// `LevelChunkSection.hasOnlyAir` of section `sy` in column `(cx, cz)`; `None` unloaded or out of range.
    pub(crate) fn only_air(&self, [cx, sy, cz]: [i32; 3]) -> Option<bool> {
        let section = usize::try_from(sy - (self.min_y >> 4)).ok()?;
        self.columns.get(&(cx, cz))?.get(section).map(|s| s.non_empty == 0)
    }

    pub(crate) fn insert(&mut self, x: i32, z: i32, sections: Vec<Section>) {
        self.columns.insert((x, z), Arc::new(sections));
    }

    pub(crate) fn remove(&mut self, x: i32, z: i32) {
        self.columns.remove(&(x, z));
    }
}

#[cfg(test)]
mod tests {
    use acacia_java_proto::chunk::Values;

    use super::*;

    #[test]
    fn snapshots_keep_their_blocks() {
        let mut chunks = Chunks::new(-64);
        chunks.insert(0, 0, vec![Section { non_empty: 4096, fluids: 0, blocks: Values::Single(1), biomes: Values::Single(0) }; 24]);
        let snapshot = chunks.clone();
        chunks.set_block([1, -64, 1], 9);
        assert_eq!(chunks.block([1, -64, 1]), Some(9));
        assert_eq!(snapshot.block([1, -64, 1]), Some(1));
        assert_eq!(snapshot.block([16, -64, 0]), None);
    }
}
