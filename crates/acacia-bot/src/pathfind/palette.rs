//! An edition's block table as the search reads it: [`Terrain`](super::Terrain) sees blocks only through
//! [`Blocks`](super::Blocks) ids and this. Bedrock's is the runtime-id [`BlockRegistry`].

use acacia_world::neutral::prop;
use acacia_world::{BlockRegistry, BlockState};

pub trait Palette {
    /// The block behind a native id: collision boxes, flags, neutral state, mining.
    fn state(&self, id: u32) -> Option<&BlockState>;
    /// The id of the same block with `Open` flipped (doors, gates, trapdoors).
    fn opened(&self, id: u32) -> Option<u32>;
}

impl Palette for BlockRegistry {
    fn state(&self, id: u32) -> Option<&BlockState> {
        self.get(id)
    }

    fn opened(&self, id: u32) -> Option<u32> {
        let n = self.get(id)?.neutral?;
        self.runtime_id_of(n.with(prop::Open, !n.get(prop::Open)?)?)
    }
}
