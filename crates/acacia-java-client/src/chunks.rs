//! Loaded chunk columns by state id. Columns are shared, so a clone is a cheap snapshot for work off the tick.

use std::collections::HashMap;
use std::sync::Arc;

use acacia_java_proto::chunk::Section;

#[derive(Debug, Clone, Default)]
pub struct Chunks {
    columns: HashMap<(i32, i32), Arc<Vec<Section>>>,
    min_y: i32,
}

impl Chunks {
    pub(crate) fn new(min_y: i32) -> Self {
        Self { columns: HashMap::new(), min_y }
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

    /// Copies the column first when a snapshot still holds it.
    pub(crate) fn set_block(&mut self, pos: [i32; 3], state: u32) {
        let Some((section, index)) = self.locate(pos) else { return };
        if let Some(s) = self.columns.get_mut(&(pos[0] >> 4, pos[2] >> 4)).and_then(|c| Arc::make_mut(c).get_mut(section)) {
            s.set_block(index, state);
        }
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
