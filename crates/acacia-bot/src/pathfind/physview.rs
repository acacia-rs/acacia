//! The search's [`Terrain`] as a physics [`WorldView`]: rollouts read the same memoized cells the moves do,
//! with the same block physics as the live simulation (`world::block_physics`).

use acacia_physics::{Aabb, BlockPhysics, BlockPos, WorldView};

use super::cell::UNLOADED;
use super::terrain::{Blocks, Terrain};
use crate::world::block_physics;

impl<B: Blocks> WorldView for Terrain<'_, B> {
    fn block_collisions(&self, pos: BlockPos, out: &mut Vec<Aabb>) {
        let c = self.cell(pos);
        if !c.has(UNLOADED) {
            out.extend(c.boxes.iter().map(|b| Aabb { min: b.min, max: b.max }));
        }
    }

    fn block(&self, pos: BlockPos) -> BlockPhysics {
        let c = self.cell(pos);
        if c.has(UNLOADED) {
            return BlockPhysics::AIR;
        }
        self.cells.physics(c, || block_physics(self.palette(), c.block, || c.liquid))
    }

    /// Loaded chunk by chunk, like the live world: a column's cells all load together.
    fn is_area_loaded(&self, area: &Aabb) -> bool {
        let y = area.min[1].floor() as i32;
        let (x0, x1) = ((area.min[0].floor() as i32) >> 4, (area.max[0].floor() as i32) >> 4);
        let (z0, z1) = ((area.min[2].floor() as i32) >> 4, (area.max[2].floor() as i32) >> 4);
        (x0..=x1).all(|cx| (z0..=z1).all(|cz| !self.cell([cx << 4, y, cz << 4]).has(UNLOADED)))
    }
}
