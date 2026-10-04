//! Controls for the motions placing a block needs, shared by the navigator and headless tests.

use acacia_physics::BlockPos;

use super::follow::Sense;
use crate::movement::Controls;

/// How far past the block centre (towards the edge) the bot sneaks before bridging: the body then
/// overhangs the edge, so the side face of the block underneath is in view.
const EDGE_OVERHANG: f32 = 0.7;
/// Feet clearance over the filled cell's top before a pillar block is placed.
const PILLAR_CLEARANCE: f32 = 0.05;

/// Bridging from `from` (the feet cell) towards `(dx, dz)`: sneaks forward until overhanging the
/// edge. True once in place.
pub(crate) fn to_edge(s: &Sense, from: BlockPos, (dx, dz): (i32, i32), c: &mut Controls) -> bool {
    let along = (s.pos[0] - from[0] as f32 - 0.5) * dx as f32 + (s.pos[2] - from[2] as f32 - 0.5) * dz as f32;
    c.stop();
    c.sneak = true;
    c.yaw = (-(dx as f32)).atan2(dz as f32).to_degrees();
    if along >= EDGE_OVERHANG {
        return true;
    }
    c.forward = 1.0;
    false
}

/// Pillaring into `cell`: jumps in place. True once the feet are above the cell.
pub(crate) fn rise(s: &Sense, cell: BlockPos, c: &mut Controls) -> bool {
    c.stop();
    c.sneak = false;
    if s.pos[1] >= (cell[1] + 1) as f32 + PILLAR_CLEARANCE {
        return true;
    }
    c.jump = s.on_ground;
    false
}
