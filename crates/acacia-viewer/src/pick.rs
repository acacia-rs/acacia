//! What the crosshair points at: the first block whose outline boxes the look ray enters.

use acacia_bot::interact::Face;
use acacia_render::blocks::{BlockTable, Shape};
use acacia_world::{BlockState, World};
use glam::{DVec3, IVec3, Vec3};

/// A block under the crosshair.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub block: IVec3,
    /// The face the ray entered through.
    pub face: Face,
    /// Outline boxes relative to the block's corner, `[min, max]` flattened.
    pub boxes: Vec<[f32; 6]>,
    /// From the eye to where the ray enters.
    pub distance: f64,
}

/// An entity the crosshair can hit, where it is drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityBox {
    pub runtime_id: u64,
    pub feet: DVec3,
    pub width: f32,
    pub height: f32,
}

/// The nearest entity whose box the ray from `eye` along `dir` (normalised) enters within `reach`,
/// and how far along.
pub fn pick_entity(entities: &[EntityBox], eye: DVec3, dir: Vec3, reach: f32) -> Option<(u64, f64)> {
    let dir = dir.as_dvec3();
    entities
        .iter()
        .filter_map(|e| {
            let half = f64::from(e.width) / 2.0;
            let min = e.feet - DVec3::new(half, 0.0, half);
            let size = DVec3::new(f64::from(e.width), f64::from(e.height), f64::from(e.width)).as_vec3();
            let (t, _) = ray_box(eye - min, dir, &[0.0, 0.0, 0.0, size.x, size.y, size.z])?;
            (t <= f64::from(reach)).then_some((e.runtime_id, t))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

const FULL: [f32; 6] = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
/// Java's short-grass and flower outline, near enough for every cross-shaped plant.
const CROSS: [f32; 6] = [2.0 / 16.0, 0.0, 2.0 / 16.0, 14.0 / 16.0, 13.0 / 16.0, 14.0 / 16.0];

/// The first targetable block along `dir` (normalised) from `eye`, at most `reach` blocks away.
pub fn pick(world: &World, table: &BlockTable, eye: DVec3, dir: Vec3, reach: f32) -> Option<Target> {
    let dir = dir.as_dvec3();
    let mut cell = eye.floor().as_ivec3();
    let step = dir.signum().as_ivec3();
    let next_edge = |p: f64, d: f64| if d > 0.0 { p.floor() + 1.0 - p } else { p - p.floor() };
    let span = |d: f64| if d == 0.0 { f64::INFINITY } else { 1.0 / d.abs() };
    let delta = DVec3::new(span(dir.x), span(dir.y), span(dir.z));
    let mut t_max = DVec3::new(next_edge(eye.x, dir.x) * delta.x, next_edge(eye.y, dir.y) * delta.y, next_edge(eye.z, dir.z) * delta.z);
    let mut travelled = 0.0;
    while travelled <= f64::from(reach) {
        if let Some(hit) = hit_in(world, table, cell, eye, dir, reach) {
            return Some(hit);
        }
        let axis = if t_max.x < t_max.y && t_max.x < t_max.z { 0 } else if t_max.y < t_max.z { 1 } else { 2 };
        travelled = t_max[axis];
        cell[axis] += step[axis];
        t_max[axis] += delta[axis];
    }
    None
}

fn hit_in(world: &World, table: &BlockTable, cell: IVec3, eye: DVec3, dir: DVec3, reach: f32) -> Option<Target> {
    let chunk = world.get(cell.x >> 4, cell.z >> 4)?;
    let id = chunk.read().block(cell.x, cell.y, cell.z);
    let state = world.registry().get(id)?;
    let boxes = outline_boxes(state, &table.get(id).shape);
    let origin = eye - cell.as_dvec3();
    let (t, axis) = boxes.iter().filter_map(|b| ray_box(origin, dir, b)).min_by(|a, b| a.0.total_cmp(&b.0))?;
    (t <= f64::from(reach)).then(|| Target { block: cell, face: entered(axis, dir[axis]), boxes, distance: t })
}

/// Collision boxes when the block has them, else what its drawn shape covers.
pub fn outline_boxes(state: &BlockState, shape: &Shape) -> Vec<[f32; 6]> {
    if state.is_air() || state.is_liquid() {
        return Vec::new();
    }
    if !state.boxes.is_empty() {
        return state.boxes.iter().map(|b| [b.min[0], b.min[1], b.min[2], b.max[0], b.max[1], b.max[2]]).collect();
    }
    match shape {
        Shape::Liquid => Vec::new(),
        Shape::Cube | Shape::None => vec![FULL],
        Shape::Cross => vec![CROSS],
        Shape::Boxes(boxes) => boxes.iter().map(|b| b.map(|v| f32::from(v) / 16.0)).collect(),
        Shape::Model(faces) => {
            let corners = faces.iter().flat_map(|f| {
                let [a, b, d] = f.corners.map(Vec3::from);
                [a, b, d, b + d - a]
            });
            let (min, max) = corners.fold((Vec3::splat(16.0), Vec3::ZERO), |(lo, hi), c| (lo.min(c), hi.max(c)));
            let (min, max) = ((min / 16.0).clamp(Vec3::ZERO, Vec3::ONE), (max / 16.0).clamp(Vec3::ZERO, Vec3::ONE));
            if min.cmpge(max).any() { vec![FULL] } else { vec![[min.x, min.y, min.z, max.x, max.y, max.z]] }
        }
    }
}

/// Where the ray enters the box and through which axis; `None` if it misses or starts inside.
fn ray_box(origin: DVec3, dir: DVec3, b: &[f32; 6]) -> Option<(f64, usize)> {
    let (mut t_in, mut t_out, mut axis) = (f64::NEG_INFINITY, f64::INFINITY, 1);
    for i in 0..3 {
        let (lo, hi) = (f64::from(b[i]), f64::from(b[i + 3]));
        if dir[i].abs() < 1e-12 {
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
    (t_in <= t_out && t_in >= 0.0).then_some((t_in, axis))
}

fn entered(axis: usize, d: f64) -> Face {
    match (axis, d > 0.0) {
        (0, true) => Face::West,
        (0, false) => Face::East,
        (1, true) => Face::Down,
        (1, false) => Face::Up,
        (2, true) => Face::North,
        _ => Face::South,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_enters_the_facing_side() {
        let down = DVec3::new(0.0, -1.0, 0.0);
        let (t, axis) = ray_box(DVec3::new(0.5, 2.0, 0.5), down, &FULL).unwrap();
        assert_eq!((t, entered(axis, down[axis])), (1.0, Face::Up));
        let east = DVec3::X;
        let (_, axis) = ray_box(DVec3::new(-1.0, 0.5, 0.5), east, &FULL).unwrap();
        assert_eq!(entered(axis, east[axis]), Face::West);
        assert!(ray_box(DVec3::new(-1.0, 1.5, 0.5), east, &FULL).is_none(), "passes over");
        assert!(ray_box(DVec3::new(0.5, 0.5, 0.5), east, &FULL).is_none(), "starts inside");
    }

    #[test]
    fn the_nearer_entity_is_hit() {
        let at = |id, x| EntityBox { runtime_id: id, feet: DVec3::new(x, 0.0, 0.0), width: 0.6, height: 1.8 };
        let eye = DVec3::new(0.0, 1.0, 0.0);
        let (id, t) = pick_entity(&[at(1, 2.5), at(2, 1.5)], eye, Vec3::X, 3.0).unwrap();
        assert_eq!(id, 2);
        assert!((t - 1.2).abs() < 1e-6, "{t}");
        assert_eq!(pick_entity(&[at(1, 4.0)], eye, Vec3::X, 3.0), None, "out of reach");
        assert_eq!(pick_entity(&[at(1, 2.0)], eye, Vec3::NEG_X, 3.0), None, "behind");
    }

    #[test]
    fn slab_outline_is_its_collision() {
        let registry = acacia_world::BlockRegistry::vanilla();
        let slab = (0..registry.len() as u32).filter_map(|id| registry.get(id)).find(|s| s.name.ends_with("stone_block_slab") || s.name == "minecraft:smooth_stone_slab").unwrap();
        let boxes = outline_boxes(slab, &Shape::Cube);
        assert_eq!(boxes.len(), 1);
        assert!(boxes[0][4] <= 0.5 + 1e-6 || boxes[0][1] >= 0.5 - 1e-6, "{} {boxes:?}", slab.name);
    }
}
