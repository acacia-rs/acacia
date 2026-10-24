//! The move of a box that is not a player's (items, orbs, falling blocks, mobs): a player's clip, without
//! step-up, edge avoidance or the per-player rules (scaffolding).

use crate::aabb::Aabb;
use crate::clip::clip_all;
use crate::math::{Vec3, add};
use crate::world::WorldView;

/// How far `bb` gets when moved by `motion` through the blocks of `w`: Y, then X, then Z.
pub fn collide_box<W: WorldView + ?Sized>(w: &W, bb: &Aabb, motion: Vec3) -> Vec3 {
    let mut boxes = Vec::new();
    w.collisions(&bb.extend(motion), &mut boxes);
    boxes.retain(|b| !b.is_empty());
    let mut bb = *bb;
    let mut moved = [0.0; 3];
    for axis in [1, 0, 2] {
        let mut along = [0.0; 3];
        along[axis] = motion[axis];
        let step = clip_all(&boxes, &bb, along, true, None);
        bb = bb.translate(step);
        moved = add(moved, step);
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_world::{TestBlock, TestWorld};

    fn world() -> TestWorld {
        let mut w = TestWorld::new();
        w.fill(Some(TestBlock::Stone), [-4, 64, -4], [4, 64, 4]).set([2, 65, 0], TestBlock::Slab).set([1, 65, 3], TestBlock::Stone);
        w
    }

    fn cube(feet: Vec3, size: f32) -> Aabb {
        let h = size / 2.0;
        Aabb::new(feet[0] - h, feet[1], feet[2] - h, feet[0] + h, feet[1] + size, feet[2] + h)
    }

    #[test]
    fn a_fall_stops_on_the_top_face() {
        let w = world();
        assert_eq!(collide_box(&w, &cube([0.5, 66.0, 0.5], 0.98), [0.0, -3.0, 0.0]), [0.0, -1.0, 0.0]);
        assert_eq!(collide_box(&w, &cube([2.5, 66.0, 0.5], 0.25), [0.0, -3.0, 0.0])[1], -0.5, "a slab's top is half a block up");
    }

    #[test]
    fn a_wall_stops_one_axis_and_the_others_go_on() {
        let moved = collide_box(&world(), &cube([0.5, 65.0, 3.5], 0.25), [0.5, 0.0, 0.2]);
        assert!((moved[0] - 0.375).abs() < 1e-6 && moved[2] == 0.2, "{moved:?}");
    }
}
