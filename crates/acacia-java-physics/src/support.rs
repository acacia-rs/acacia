use acacia_registry::{Block, BlockTag};

use crate::aabb::Aabb;
use crate::block_factors::speed_factor;
use crate::collide::{Blocks, Colliders};
use crate::mth::floor;

pub(crate) const BELOW_MOVEMENT_OFFSET: f32 = 0.500001;
pub(crate) const LEGACY_ON_POS: f32 = 0.2;

/// `Entity.mainSupportingBlockPos`: the block an entity stands on, which `getOnPos` prefers.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Support {
    block: Option<[i32; 3]>,
    no_blocks: bool,
}

impl Support {
    /// `Entity.checkSupportingBlock`.
    pub(crate) fn update<W: Blocks>(&mut self, colliders: &Colliders<W>, bbox: Aabb, pos: [f64; 3], on_ground: bool, movement: [f64; 3]) {
        if !on_ground {
            *self = Support::default();
            return;
        }
        let area = Aabb { min: [bbox.min[0], bbox.min[1] - 1.0E-6, bbox.min[2]], max: [bbox.max[0], bbox.min[1], bbox.max[2]] };
        let mut found = colliders.supporting_block(&area, pos);
        if found.is_none() && !self.no_blocks {
            found = colliders.supporting_block(&area.moved([-movement[0], 0.0, -movement[2]]), pos);
        }
        *self = Support { block: found, no_blocks: found.is_none() };
    }

    /// `Entity.getOnPos` for offsets above 1.0E-5.
    pub(crate) fn on_pos(&self, world: &impl Blocks, pos: [f64; 3], offset: f32) -> [i32; 3] {
        let below = floor(pos[1] - offset as f64);
        let Some(support) = self.block else {
            return [floor(pos[0]), below, floor(pos[2])];
        };
        let block = world.block_state(support).block();
        let keeps_support = (offset as f64 <= 0.5 && block.is(BlockTag::Fences)) || block.is(BlockTag::Walls) || block.is(BlockTag::FenceGates);
        if keeps_support { support } else { [support[0], below, support[2]] }
    }

    /// `Entity.getBlockSpeedFactor`.
    pub(crate) fn speed_factor(&self, world: &impl Blocks, pos: [f64; 3]) -> f32 {
        let block = world.block_state(pos.map(floor)).block();
        let here = speed_factor(block);
        if block == Block::Water || block == Block::BubbleColumn || here as f64 != 1.0 {
            return here;
        }
        speed_factor(world.block_state(self.on_pos(world, pos, BELOW_MOVEMENT_OFFSET)).block())
    }
}
