//! The geometry of a shulker's lid, shared by the mob and the box block: `Shulker.getPhysicalPeek` and
//! `getProgressDeltaAabb`. Directions are 3D data values (down, up, north, south, west, east).

use acacia_java_physics::{Aabb, sin};

/// `Direction.getStepX/Y/Z`.
pub(crate) fn step(direction: i32) -> [f64; 3] {
    match direction {
        0 => [0.0, -1.0, 0.0],
        1 => [0.0, 1.0, 0.0],
        2 => [0.0, 0.0, -1.0],
        3 => [0.0, 0.0, 1.0],
        4 => [-1.0, 0.0, 0.0],
        _ => [1.0, 0.0, 0.0],
    }
}

pub(crate) fn opposite(direction: i32) -> i32 {
    direction ^ 1
}

/// `Shulker.getPhysicalPeek`: how far out the lid sits, eased from the peek amount.
pub(crate) fn physical_peek(amount: f32) -> f32 {
    0.5 - sin(f64::from((0.5 + amount) * std::f32::consts::PI)) * 0.5
}

/// `getProgressDeltaAabb`: the slab a lid sweeps between two progresses, out of a `size` cube standing at `pos`;
/// `from` of -1 gives the whole body (`getProgressAabb`).
pub(crate) fn progress_box(size: f32, direction: i32, from: f32, to: f32, pos: [f64; 3]) -> Aabb {
    let s = step(direction);
    let (size, hi, lo) = (f64::from(size), f64::from(from.max(to)), f64::from(from.min(to)));
    let (mut min, mut max) = ([-size * 0.5, 0.0, -size * 0.5], [size * 0.5, size, size * 0.5]);
    for a in 0..3 {
        let grow = s[a] * hi * size;
        if grow < 0.0 {
            min[a] += grow;
        } else if grow > 0.0 {
            max[a] += grow;
        }
        let shrink = -s[a] * (1.0 + lo) * size;
        if shrink < 0.0 {
            min[a] -= shrink;
        } else if shrink > 0.0 {
            max[a] -= shrink;
        }
    }
    Aabb { min: [min[0] + pos[0], min[1] + pos[1], min[2] + pos[2]], max: [max[0] + pos[0], max[1] + pos[1], max[2] + pos[2]] }
}
