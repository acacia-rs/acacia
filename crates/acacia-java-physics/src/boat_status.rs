//! `AbstractBoat.getStatus` and the water scans behind it.

use acacia_registry::Block;

use crate::aabb::Aabb;
use crate::block_factors::friction;
use crate::boat::{Boat, BoatStatus};
use crate::collide::Blocks;
use crate::fluid::{fluid_at, height};
use crate::mth::{ceil, floor};

/// Water's surface in a cell as vanilla adds it: `y + getHeight` in floats.
fn water_top(world: &impl Blocks, pos: [i32; 3]) -> Option<f32> {
    fluid_at(world, pos).filter(|f| !f.lava).map(|f| pos[1] as f32 + height(world, pos, f))
}

impl Boat {
    /// `getStatus`; updates the water level and land friction as vanilla does on the way.
    pub(crate) fn read_status(&mut self, world: &impl Blocks) -> BoatStatus {
        let b = self.bounding_box();
        if let Some(status) = under_water(world, &b) {
            self.water_level = b.max[1];
            return status;
        }
        if self.check_in_water(world, &b) {
            return BoatStatus::InWater;
        }
        let friction = ground_friction(world, &b);
        if friction > 0.0 {
            self.land_friction = friction;
            BoatStatus::OnLand
        } else {
            BoatStatus::InAir
        }
    }

    fn check_in_water(&mut self, world: &impl Blocks, b: &Aabb) -> bool {
        let mut in_water = false;
        self.water_level = -f64::MAX;
        for x in floor(b.min[0])..ceil(b.max[0]) {
            for y in floor(b.min[1])..ceil(b.min[1] + 0.001) {
                for z in floor(b.min[2])..ceil(b.max[2]) {
                    if let Some(top) = water_top(world, [x, y, z]) {
                        self.water_level = (top as f64).max(self.water_level);
                        in_water |= b.min[1] < top as f64;
                    }
                }
            }
        }
        in_water
    }

    /// `getWaterLevelAbove`: the first layer from the boat's top up that water doesn't fill.
    pub(crate) fn water_level_above(&self, world: &impl Blocks) -> f32 {
        let b = self.bounding_box();
        let max_y = ceil(b.max[1] - self.last_yd);
        for y in floor(b.max[1])..max_y {
            let mut block_height = 0.0_f32;
            'layer: for x in floor(b.min[0])..ceil(b.max[0]) {
                for z in floor(b.min[2])..ceil(b.max[2]) {
                    if let Some(f) = fluid_at(world, [x, y, z]).filter(|f| !f.lava) {
                        block_height = block_height.max(height(world, [x, y, z], f));
                    }
                    if block_height >= 1.0 {
                        break 'layer;
                    }
                }
            }
            if block_height < 1.0 {
                return y as f32 + block_height;
            }
        }
        (max_y + 1) as f32
    }
}

/// `isUnderwater`: water over the boat's top, flowing water first.
fn under_water(world: &impl Blocks, b: &Aabb) -> Option<BoatStatus> {
    let top = b.max[1] + 0.001;
    let mut under = false;
    for x in floor(b.min[0])..ceil(b.max[0]) {
        for y in floor(b.max[1])..ceil(top) {
            for z in floor(b.min[2])..ceil(b.max[2]) {
                let pos = [x, y, z];
                let Some(f) = fluid_at(world, pos).filter(|f| !f.lava) else { continue };
                if top < (y as f32 + height(world, pos, f)) as f64 {
                    if !f.source {
                        return Some(BoatStatus::UnderFlowingWater);
                    }
                    under = true;
                }
            }
        }
    }
    under.then_some(BoatStatus::UnderWater)
}

/// `getGroundFriction`: mean friction of the blocks under the boat, x then z then y, skipping the corner columns
/// and the edge columns' top and bottom layers; NaN when none.
fn ground_friction(world: &impl Blocks, b: &Aabb) -> f32 {
    let slab = Aabb { min: [b.min[0], b.min[1] - 0.001, b.min[2]], max: [b.max[0], b.min[1], b.max[2]] };
    let (x0, x1) = (floor(slab.min[0]) - 1, ceil(slab.max[0]) + 1);
    let (y0, y1) = (floor(slab.min[1]) - 1, ceil(slab.max[1]) + 1);
    let (z0, z1) = (floor(slab.min[2]) - 1, ceil(slab.max[2]) + 1);
    let mut total = 0.0_f32;
    let mut count = 0;
    for x in x0..x1 {
        for z in z0..z1 {
            let edges = u8::from(x == x0 || x == x1 - 1) + u8::from(z == z0 || z == z1 - 1);
            if edges == 2 {
                continue;
            }
            for y in y0..y1 {
                if edges > 0 && (y == y0 || y == y1 - 1) {
                    continue;
                }
                let state = world.block_state([x, y, z]);
                let touches = state.java_collision().is_some_and(|boxes| {
                    boxes.iter().any(|c| {
                        let at = [x as f64, y as f64, z as f64];
                        Aabb { min: [c[0] + at[0], c[1] + at[1], c[2] + at[2]], max: [c[3] + at[0], c[4] + at[1], c[5] + at[2]] }.intersects(&slab)
                    })
                });
                if state.block() != Block::LilyPad && touches {
                    total += friction(state.block());
                    count += 1;
                }
            }
        }
    }
    total / count as f32
}
