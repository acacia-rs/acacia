//! `Entity.move` for `MoverType.SHULKER` or `SHULKER_BOX` on an entity the client only shows (a vehicle someone else
//! drives, a minecart): its box collides with blocks and `solid` boxes, stepping up to `step` from the ground.

use crate::aabb::Aabb;
use crate::collide::{Blocks, Colliders, Context};
use crate::mth::length_sqr;

/// The entity's new position after a shove by `delta`, from `pos` with a `size` (width, height) box.
pub fn shove(world: &impl Blocks, pos: [f64; 3], size: (f32, f32), on_ground: bool, step: f32, solid: &[Aabb], delta: [f64; 3]) -> [f64; 3] {
    let colliders = Colliders { world, context: Context { feet: pos[1], ..Context::default() }, entities: solid };
    let movement = colliders.collide(Aabb::of_entity(pos, size.0, size.1), delta, on_ground, step);
    let moved = length_sqr(movement);
    if moved > 1.0E-7 || length_sqr(delta) - moved < 1.0E-7 {
        [pos[0] + movement[0], pos[1] + movement[1], pos[2] + movement[2]]
    } else {
        pos
    }
}
