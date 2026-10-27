//! What the crosshair points at: the first block whose outline boxes the look ray enters.

use acacia_bot::interact::Face;
use acacia_render::blocks::{BlockTable, Shape};
use acacia_world::{ray, Aabb, BlockAccess, BlockState, World};
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
            let size = Aabb::new([0.0; 3], [e.width, e.height, e.width]);
            let (t, _) = ray::enter((eye - min).to_array(), dir.to_array(), &size)?;
            (t <= f64::from(reach)).then_some((e.runtime_id, t))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

const FULL: [f32; 6] = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
/// Java's short-grass and flower outline, near enough for every cross-shaped plant.
const CROSS: [f32; 6] = [2.0 / 16.0, 0.0, 2.0 / 16.0, 14.0 / 16.0, 13.0 / 16.0, 14.0 / 16.0];

/// The first targetable block along `dir` (normalised) from `eye`, at most `reach` blocks away.
pub fn pick(world: &World, table: &BlockTable, eye: DVec3, dir: Vec3, reach: f32) -> Option<Target> {
    let registry = world.registry();
    let outline = |id: u32| registry.get(id).map_or_else(Vec::new, |state| outline_boxes(state, &table.get(id).shape));
    let hit = ray::cast(&Loaded(world), eye.to_array(), dir.as_dvec3().to_array(), f64::from(reach), |_, id, out| {
        out.extend(outline(id).iter().map(|b| Aabb::new([b[0], b[1], b[2]], [b[3], b[4], b[5]])));
    })?;
    let id = Loaded(world).block(hit.block[0], hit.block[1], hit.block[2]);
    Some(Target { block: IVec3::from_array(hit.block), face: Face::from_wire(hit.face), boxes: outline(id), distance: hit.distance })
}

/// The viewer's world as the raycast reads it: unloaded chunks are air.
struct Loaded<'a>(&'a World);

impl BlockAccess for Loaded<'_> {
    fn block(&self, x: i32, y: i32, z: i32) -> u32 {
        self.0.get(x >> 4, z >> 4).map_or_else(|| self.0.registry().air_id(), |chunk| chunk.read().block(x, y, z))
    }

    fn liquid(&self, _: i32, _: i32, _: i32) -> u32 {
        self.0.registry().air_id()
    }
}

/// Collision boxes when the block has them, else what its drawn shape covers.
pub fn outline_boxes(state: &BlockState, shape: &Shape) -> Vec<[f32; 6]> {
    if state.is_air() || state.is_liquid() {
        return Vec::new();
    }
    if !state.boxes.is_empty() {
        let mut boxes: Vec<[f32; 6]> = state.boxes.iter().map(|b| [b.min[0], b.min[1], b.min[2], b.max[0], b.max[1], b.max[2]]).collect();
        // A wall hanging sign collides with its bracket only; the board under it is clicked too.
        if state.name.ends_with("hanging_sign") {
            boxes.extend(boxes.first().map(|bar| [bar[0], 0.0, bar[2], bar[3], bar[1], bar[5]]));
        }
        return boxes;
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn a_wall_hanging_sign_is_clicked_on_its_board() {
        let registry = acacia_world::BlockRegistry::vanilla();
        let signs = (0..registry.len() as u32).filter_map(|id| registry.get(id)).filter(|s| s.name.ends_with("oak_hanging_sign") && !s.boxes.is_empty());
        let mut seen = 0;
        for sign in signs {
            seen += 1;
            assert!(outline_boxes(sign, &Shape::None).iter().any(|b| b[1] == 0.0 && b[4] > 0.5), "{:?}", sign.boxes);
        }
        assert!(seen > 0, "no hanging sign collides");
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
