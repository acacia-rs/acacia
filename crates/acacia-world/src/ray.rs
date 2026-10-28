//! Block raycast: the first block shape a ray enters, walking cells in order (Amanatides–Woo).

use crate::{Aabb, BlockAccess};

/// Where a ray first entered a block shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    pub block: [i32; 3],
    /// Face entered, numbered as on the wire: 0 down, 1 up, 2 north, 3 south, 4 west, 5 east.
    pub face: u8,
    /// Distance from the origin along the (normalised) direction; 0 when the origin is inside the shape.
    pub distance: f64,
    pub point: [f64; 3],
}

/// The first shape the ray from `origin` along `dir` (normalised) enters within `reach`. `shape` fills the
/// block-local boxes of the block `id` at a position; an empty list lets the ray through.
pub fn cast(
    world: &impl BlockAccess,
    origin: [f64; 3],
    dir: [f64; 3],
    reach: f64,
    mut shape: impl FnMut([i32; 3], u32, &mut Vec<Aabb>),
) -> Option<RayHit> {
    let mut cell = origin.map(|c| c.floor() as i32);
    let step = dir.map(|d| if d > 0.0 { 1 } else if d < 0.0 { -1 } else { 0 });
    let delta = dir.map(|d| if d == 0.0 { f64::INFINITY } else { 1.0 / d.abs() });
    let mut t_max: [f64; 3] = std::array::from_fn(|i| {
        let p = origin[i];
        let edge = if dir[i] > 0.0 { p.floor() + 1.0 - p } else { p - p.floor() };
        edge * delta[i]
    });
    let mut boxes = Vec::new();
    let mut travelled = 0.0;
    while travelled <= reach {
        if let Some(hit) = hit_in(world, cell, origin, dir, &mut boxes, &mut shape).filter(|h| h.distance <= reach) {
            return Some(hit);
        }
        let axis = if t_max[0] < t_max[1] && t_max[0] < t_max[2] { 0 } else if t_max[1] < t_max[2] { 1 } else { 2 };
        travelled = t_max[axis];
        cell[axis] += step[axis];
        t_max[axis] += delta[axis];
    }
    None
}

/// The nearest entry into the shapes occupying `cell`: its own block's, and the part of the block below
/// that rises past its top (fences and walls are 1.5 high).
fn hit_in(
    world: &impl BlockAccess,
    cell: [i32; 3],
    origin: [f64; 3],
    dir: [f64; 3],
    boxes: &mut Vec<Aabb>,
    shape: &mut impl FnMut([i32; 3], u32, &mut Vec<Aabb>),
) -> Option<RayHit> {
    let below = [cell[0], cell[1] - 1, cell[2]];
    let mut best: Option<RayHit> = None;
    for (owner, tall_only) in [(cell, false), (below, true)] {
        boxes.clear();
        shape(owner, world.block(owner[0], owner[1], owner[2]), boxes);
        let local: [f64; 3] = std::array::from_fn(|i| origin[i] - f64::from(owner[i]));
        for b in boxes.iter().filter(|b| !tall_only || b.max[1] > 1.0) {
            let Some((t, axis)) = enter(local, dir, b) else { continue };
            if best.is_none_or(|h| t < h.distance) {
                let point = std::array::from_fn(|i| origin[i] + dir[i] * t);
                best = Some(RayHit { block: owner, face: face(axis, dir[axis]), distance: t, point });
            }
        }
    }
    best
}

/// Where the ray enters the box and through which axis; `(0, _)` when it starts inside.
pub fn enter(origin: [f64; 3], dir: [f64; 3], b: &Aabb) -> Option<(f64, usize)> {
    let (mut t_in, mut t_out, mut axis) = (f64::NEG_INFINITY, f64::INFINITY, 1);
    for i in 0..3 {
        let (lo, hi) = (f64::from(b.min[i]), f64::from(b.max[i]));
        if dir[i] == 0.0 {
            if origin[i] < lo || origin[i] > hi {
                return None;
            }
            continue;
        }
        let (a, c) = ((lo - origin[i]) / dir[i], (hi - origin[i]) / dir[i]);
        let (near, far) = if a < c { (a, c) } else { (c, a) };
        if near > t_in {
            (t_in, axis) = (near, i);
        }
        t_out = t_out.min(far);
    }
    (t_in <= t_out && t_out >= 0.0).then_some((t_in.max(0.0), axis))
}

/// The face a ray moving along `d` on `axis` enters through.
fn face(axis: usize, d: f64) -> u8 {
    match (axis, d > 0.0) {
        (0, true) => 4,
        (0, false) => 5,
        (1, true) => 0,
        (1, false) => 1,
        (2, true) => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Cells(HashMap<[i32; 3], u32>);

    impl BlockAccess for Cells {
        fn block(&self, x: i32, y: i32, z: i32) -> u32 {
            self.0.get(&[x, y, z]).copied().unwrap_or(0)
        }
        fn liquid(&self, _: i32, _: i32, _: i32) -> u32 {
            0
        }
    }

    const FENCE: Aabb = Aabb::new([0.375, 0.0, 0.375], [0.625, 1.5, 0.625]);

    fn shapes(_: [i32; 3], id: u32, out: &mut Vec<Aabb>) {
        match id {
            1 => out.push(Aabb::FULL),
            2 => out.push(FENCE),
            _ => {}
        }
    }

    fn world(cells: &[([i32; 3], u32)]) -> Cells {
        Cells(cells.iter().copied().collect())
    }

    #[test]
    fn stops_at_the_first_block_and_names_the_face() {
        let w = world(&[([3, 0, 0], 1), ([5, 0, 0], 1)]);
        let hit = cast(&w, [0.5, 0.5, 0.5], [1.0, 0.0, 0.0], 10.0, shapes).unwrap();
        assert_eq!((hit.block, hit.face), ([3, 0, 0], 4));
        assert!((hit.distance - 2.5).abs() < 1e-9);
    }

    #[test]
    fn misses_beyond_reach_and_through_empty_shapes() {
        let w = world(&[([3, 0, 0], 1), ([1, 0, 0], 7)]);
        assert_eq!(cast(&w, [0.5, 0.5, 0.5], [1.0, 0.0, 0.0], 2.4, shapes), None);
    }

    #[test]
    fn walks_negative_coordinates_diagonally() {
        let w = world(&[([-3, -2, -3], 1)]);
        let d = 1.0 / 3f64.sqrt();
        let hit = cast(&w, [0.5, 0.5, 0.5], [-d, -d * 0.7, -d], 10.0, shapes).unwrap();
        assert_eq!(hit.block, [-3, -2, -3]);
    }

    #[test]
    fn the_top_of_a_fence_below_blocks_the_cell_above() {
        let w = world(&[([3, 0, 0], 2)]);
        let hit = cast(&w, [0.5, 1.25, 0.5], [1.0, 0.0, 0.0], 10.0, shapes).unwrap();
        assert_eq!((hit.block, hit.face), ([3, 0, 0], 4));
        assert!((hit.point[0] - 3.375).abs() < 1e-9);
    }

    #[test]
    fn starting_inside_a_shape_hits_at_zero() {
        let w = world(&[([0, 0, 0], 1)]);
        assert_eq!(cast(&w, [0.5, 0.5, 0.5], [0.0, 1.0, 0.0], 5.0, shapes).unwrap().distance, 0.0);
    }

    #[test]
    fn a_ray_along_a_face_plane_enters_the_box() {
        assert_eq!(enter([-1.0, 1.0, 0.5], [1.0, 0.0, 0.0], &Aabb::FULL), Some((1.0, 0)));
        assert_eq!(enter([0.0; 3], [-1.0, 0.0, 0.0], &Aabb::new([2.0, -1.0, -1.0], [3.0, 1.0, 1.0])), None);
    }
}
