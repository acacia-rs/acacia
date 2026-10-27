//! What the eye can see: a ray is stopped by any block shape a vanilla crosshair or an anticheat ray would
//! stop on, and the bot aims only at points it sees (research/anticheat-interaction-checks.md).

use acacia_physics::{BlockPos, Vec3};
use acacia_world::ray::{self, RayHit};
use acacia_world::{selection, Aabb, BlockAccess, BlockRegistry};

use super::geometry::Face;

/// Clicks within this of the ray's end still count as reaching it.
const SLACK: f64 = 1e-3;
/// Points tried on a box: its centre, then inset towards its corners.
const INSET: f32 = 0.2;

pub(crate) struct Sight<'a, W: BlockAccess> {
    pub world: &'a W,
    pub registry: &'a BlockRegistry,
}

impl<W: BlockAccess> Sight<'_, W> {
    /// The first block shape on the way from `eye` towards `to`, at most `extra` past it; the cell `through` is
    /// passed through.
    fn cast(&self, eye: Vec3, to: Vec3, extra: f64, through: Option<BlockPos>) -> Option<RayHit> {
        let (origin, d) = (eye.map(f64::from), [to[0] - eye[0], to[1] - eye[1], to[2] - eye[2]].map(f64::from));
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if len < 1e-9 {
            return None;
        }
        // Collision stops an anticheat's ray, selection the vanilla crosshair's: the bot respects both.
        let shapes = |pos: [i32; 3], id: u32, out: &mut Vec<Aabb>| {
            if let Some(s) = self.registry.get(id).filter(|_| through != Some(pos)) {
                out.extend(s.boxes.iter().copied().chain(selection::boxes_at(self.world, self.registry, pos)));
            }
        };
        ray::cast(self.world, origin, d.map(|c| c / len), len + extra, shapes)
    }

    /// No block shape lies between `eye` and `point` (an entity's hit point).
    pub fn clear(&self, eye: Vec3, point: Vec3) -> bool {
        self.cast(eye, point, -SLACK, None).is_none()
    }

    /// The ray from `eye` through `point` first meets the block at `pos` on `face`.
    pub fn sees_face_at(&self, eye: Vec3, pos: BlockPos, face: Face, point: Vec3) -> bool {
        self.sees_face_into(eye, pos, face, point, None)
    }

    /// [`Self::sees_face_at`] for a placement that merges into `into` (a half slab made double): what is there now
    /// does not hide the face.
    pub fn sees_face_into(&self, eye: Vec3, pos: BlockPos, face: Face, point: Vec3, into: Option<BlockPos>) -> bool {
        self.cast(eye, point, 0.05, into).is_some_and(|h| h.block == pos && Face::from_wire(h.face) == face)
    }

    /// Faces of the block at `pos` the eye sees, each with the world points on it the eye reaches unblocked,
    /// nearest to the face's centre first.
    pub fn visible_points(&self, eye: Vec3, pos: BlockPos) -> Vec<(Face, Vec3)> {
        let Some(state) = self.registry.get(self.world.block(pos[0], pos[1], pos[2])) else { return Vec::new() };
        let selection = selection::boxes_at(self.world, self.registry, pos);
        let boxes = if selection.is_empty() { state.boxes.to_vec() } else { selection };
        let mut seen = Vec::new();
        for b in &boxes {
            for p in samples(b) {
                let to = [pos[0] as f32 + p[0], pos[1] as f32 + p[1], pos[2] as f32 + p[2]];
                let Some(hit) = self.cast(eye, to, 0.05, None).filter(|h| h.block == pos) else { continue };
                let at = hit.point.map(|c| c as f32);
                if !seen.iter().any(|&(_, q): &(Face, Vec3)| q == at) {
                    seen.push((Face::from_wire(hit.face), at));
                }
            }
        }
        seen
    }
}

/// Block-local points inside `b`: its centre, then inset towards each corner.
fn samples(b: &Aabb) -> impl Iterator<Item = Vec3> + '_ {
    let c: Vec3 = std::array::from_fn(|i| (b.min[i] + b.max[i]) / 2.0);
    let corner = move |n: usize| -> Vec3 { std::array::from_fn(|i| if n >> i & 1 == 0 { b.min[i] + (c[i] - b.min[i]) * INSET } else { b.max[i] - (b.max[i] - c[i]) * INSET }) };
    std::iter::once(c).chain((0..8).map(corner))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct Cells(HashMap<BlockPos, u32>, u32);

    impl BlockAccess for Cells {
        fn block(&self, x: i32, y: i32, z: i32) -> u32 {
            self.0.get(&[x, y, z]).copied().unwrap_or(self.1)
        }
        fn liquid(&self, _: i32, _: i32, _: i32) -> u32 {
            self.1
        }
    }

    fn world(stones: &[BlockPos]) -> (Cells, &'static BlockRegistry) {
        let r = BlockRegistry::vanilla();
        let stone = r.find("minecraft:stone", "").unwrap();
        (Cells(stones.iter().map(|&p| (p, stone)).collect(), r.air_id()), r)
    }

    #[test]
    fn a_wall_hides_the_block_behind_it() {
        let (w, registry) = world(&[[2, 0, 0], [3, 0, 0]]);
        let sight = Sight { world: &w, registry };
        let eye = [0.5, 0.5, 0.5];
        assert!(sight.visible_points(eye, [3, 0, 0]).is_empty());
        let front = sight.visible_points(eye, [2, 0, 0]);
        assert!(!front.is_empty() && front.iter().all(|&(f, _)| f == Face::West));
        assert!(sight.sees_face_at(eye, [2, 0, 0], Face::West, [2.0, 0.5, 0.5]));
        assert!(!sight.sees_face_at(eye, [3, 0, 0], Face::West, [3.0, 0.5, 0.5]));
    }

    #[test]
    fn a_block_seen_from_above_shows_its_top_and_side() {
        let (w, registry) = world(&[[2, 0, 0]]);
        let sight = Sight { world: &w, registry };
        let faces: Vec<Face> = sight.visible_points([0.5, 2.6, 0.5], [2, 0, 0]).into_iter().map(|(f, _)| f).collect();
        assert!(faces.contains(&Face::Up) && faces.contains(&Face::West), "{faces:?}");
    }

    #[test]
    fn a_flower_hides_what_is_behind_it_and_can_be_aimed_at() {
        let r = BlockRegistry::vanilla();
        let (poppy, stone) = (r.find("minecraft:poppy", "").unwrap(), r.find("minecraft:stone", "").unwrap());
        let w = Cells([([2, 0, 0], poppy), ([3, 0, 0], stone)].into_iter().collect(), r.air_id());
        let sight = Sight { world: &w, registry: r };
        let eye = [0.5, 0.3, 0.45];
        assert!(!sight.clear(eye, [3.0, 0.3, 0.45]), "no collision, but the crosshair stops on it");
        assert!(!sight.visible_points(eye, [2, 0, 0]).is_empty());
    }

    #[test]
    fn an_entity_point_behind_a_wall_is_not_clear() {
        let (w, registry) = world(&[[2, 1, 0]]);
        let sight = Sight { world: &w, registry };
        assert!(!sight.clear([0.5, 1.5, 0.5], [3.5, 1.5, 0.5]));
        assert!(sight.clear([0.5, 1.5, 0.5], [1.9, 1.5, 0.5]));
    }
}
