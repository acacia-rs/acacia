//! What a block is to its neighbours' faces in Java (`Block.shouldRenderFace`): baked by
//! `lookbake java` for the Java look and checked against the game (harness `light_facts`);
//! the mesher reads it in `mesh::cull`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Cull {
    /// Its face occlusion shapes (`getFaceOcclusionShape`): an index from 1 into
    /// [`super::BlockTable::light_shapes`], 0 when it cannot occlude (`canOcclude`).
    pub shape: u16,
    /// Bit `d` set where its face shape towards Java direction `d` is the whole block
    /// (`Shapes.block()`, a single box over the side): that hides every face against it, where
    /// one that merely covers the side hides only faces that have a shape themselves.
    pub whole: u8,
    pub skip: Skip,
}

/// The block's `skipRendering`: faces it leaves out against a neighbour whatever their shapes.
/// `block` tells blocks apart (`is(this)`); a look numbers its Java blocks as it likes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Skip {
    #[default]
    None,
    /// `HalfTransparentBlock`, `PowderSnowBlock`: against the same block.
    Same { block: u16 },
    /// `MangroveRootsBlock`: against the same block, up and down.
    SameVertical { block: u16 },
    /// `IronBarsBlock`: up and down against the same block (or bars against bars, `tagged` being
    /// `#bars`); sideways where both connect towards each other. `sides` has bit `d` set for each
    /// Java direction `d` (2..=5) it connects to.
    Bars { block: u16, tagged: bool, sides: u8 },
}
