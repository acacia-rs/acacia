//! Entities the player collides with. Strict BDS makes boats solid to a player walking into or standing on one (it
//! steps up onto a resting boat), and pushes no player out of a mob: a player walking through a cow pushes the cow
//! and keeps its own path (drill `entity_drill`, evidence in README.md "Entities").

use crate::aabb::Aabb;
use crate::math::BlockPos;
use crate::world::{BlockPhysics, WorldView};

/// `world` plus the collision boxes of the solid entities around the player (world space).
pub struct WithEntities<'a, W: ?Sized> {
    pub world: &'a W,
    pub boxes: &'a [Aabb],
}

impl<W: WorldView + ?Sized> WorldView for WithEntities<'_, W> {
    fn block_collisions(&self, pos: BlockPos, out: &mut Vec<Aabb>) {
        self.world.block_collisions(pos, out);
    }

    fn block(&self, pos: BlockPos) -> BlockPhysics {
        self.world.block(pos)
    }

    fn collisions(&self, area: &Aabb, out: &mut Vec<Aabb>) {
        self.world.collisions(area, out);
        out.extend(self.boxes.iter().filter(|b| b.intersects(area)));
    }

    fn is_area_loaded(&self, area: &Aabb) -> bool {
        self.world.is_area_loaded(area)
    }
}

#[cfg(test)]
mod tests;
