//! What Java's `FluidRenderer` and `FlowingFluid.getFlow` read of a block state, derived from the
//! Bedrock state: its fluid amount, the faces its shape covers, and the block-class tests Java makes.

use acacia_world::{Aabb, BlockState};

use super::short_name;

/// Per block state; [`super::RenderBlock::fluid_rules`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FluidRules {
    /// Java's fluid amount, 1..=8 (sources and falling fluid 8); 0 for no fluid.
    pub amount: u8,
    pub falling: bool,
    /// Per face ([`crate::assets::FACE_NAMES`] order), how far from the bottom the shape covers the
    /// whole face; up and down faces are 1 or 0 (Horizon's `coverage`).
    pub cover: [f32; 6],
    /// `canOcclude`: false for the `noOcclusion` blocks (glass, leaves, doors...).
    pub occludes: bool,
    /// `BlockState.isSolid` (legacy solid: big enough collision bounds).
    pub solid: bool,
    /// In `#blocks_fluid_flow` (`#blocks_motion` and signs); approximated from the collision
    /// bounds plus the collisionless blocks the tag lists.
    pub blocks_motion: bool,
    /// `HalfTransparentBlock` or `LeavesBlock`: water beside it shows the overlay texture.
    pub overlay: bool,
    /// `IceBlock`.
    pub ice: bool,
}

/// Java blocks that call `noOcclusion` among those with collision, by Bedrock name fragment.
const NO_OCCLUSION: &[&str] = &[
    "glass", "leaves", "door", "bars", "grate", "slime", "honey_block", "scaffolding", "spawner", "beacon", "chorus", "lantern",
    "bell", "chain", "bamboo", "roots", "vault", "conduit", "barrier", "cobweb", "web",
];

impl FluidRules {
    /// A full opaque cube: what unknown blocks and unloaded neighbours are.
    pub const CUBE: FluidRules =
        FluidRules { amount: 0, falling: false, cover: [1.0; 6], occludes: true, solid: true, blocks_motion: true, overlay: false, ice: false };

    pub fn of(state: &BlockState) -> FluidRules {
        let name = short_name(state.name);
        let (amount, falling) = match state.liquid_depth {
            _ if !state.is_liquid() => (0, false),
            0 => (8, false),
            d @ 1..8 => (8 - d, false),
            _ => (8, true),
        };
        let ice = matches!(name, "ice" | "frosted_ice");
        let solid = legacy_solid(state.boxes);
        FluidRules {
            amount,
            falling,
            cover: if state.is_full_cube() { [1.0; 6] } else { std::array::from_fn(|face| coverage(state.boxes, face)) },
            occludes: !ice && !NO_OCCLUSION.iter().any(|n| name.contains(n)),
            solid,
            blocks_motion: (solid && !matches!(name, "web" | "cobweb" | "bamboo_sapling"))
                || ["sign", "banner", "pressure_plate"].iter().any(|n| name.contains(n)),
            overlay: ice || name.contains("leaves") || name == "slime" || name == "honey_block" || name.ends_with("copper_grate")
                || (name.contains("glass") && !name.contains("pane")),
            ice,
        }
    }

    /// The face `face` as an occluder: `cover` when the block can occlude.
    pub fn occluding(&self, face: usize) -> f32 {
        if self.occludes { self.cover[face] } else { 0.0 }
    }

    /// `isFaceSturdy` on `face`, from the collision boxes.
    pub fn sturdy(&self, face: usize) -> bool {
        self.cover[face] >= 1.0
    }

    /// `FluidState.getOwnHeight`.
    pub fn own_height(&self) -> f32 {
        f32::from(self.amount) / 9.0
    }
}

/// `calculateSolid`: collision bounds averaging 0.729 a side, or a full block tall.
fn legacy_solid(boxes: &[Aabb]) -> bool {
    let Some(first) = boxes.first() else { return false };
    let (mut min, mut max) = (first.min, first.max);
    for b in boxes {
        for i in 0..3 {
            min[i] = min[i].min(b.min[i]);
            max[i] = max[i].max(b.max[i]);
        }
    }
    let size = |i: usize| f64::from(max[i] - min[i]);
    (size(0) + size(1) + size(2)) / 3.0 >= 0.7291666666666666 || size(1) >= 1.0
}

/// For `face` (east, west, up, down, south, north): the height from the bottom over which the
/// boxes flush with that side cover its whole width; up and down give 1 when covered whole, else 0.
/// This is what `Shapes.blockOccludes` decides against a fluid box of a given height.
pub fn coverage(boxes: &[Aabb], face: usize) -> f32 {
    let (axis, u, v) = [(0, 2, 1), (0, 2, 1), (1, 0, 2), (1, 0, 2), (2, 0, 1), (2, 0, 1)][face];
    let positive = face.is_multiple_of(2);
    let clamp = |c: f32| f64::from(c.clamp(0.0, 1.0));
    let rects: Vec<[f64; 4]> = boxes
        .iter()
        .filter(|b| if positive { b.max[axis] >= 1.0 - 1e-7 } else { b.min[axis] <= 1e-7 })
        .map(|b| [clamp(b.min[u]), clamp(b.max[u]), clamp(b.min[v]), clamp(b.max[v])])
        .collect();
    let mut cuts: Vec<f64> = rects.iter().flat_map(|r| [r[2], r[3]]).chain([0.0, 1.0]).collect();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup();
    let mut covered = 0.0;
    for w in cuts.windows(2) {
        let middle = (w[0] + w[1]) / 2.0;
        let mut spans: Vec<(f64, f64)> = rects.iter().filter(|r| r[2] <= middle && middle <= r[3]).map(|r| (r[0], r[1])).collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut reach = 0.0f64;
        for (lo, hi) in spans {
            if lo > reach + 1e-7 {
                break;
            }
            reach = reach.max(hi);
        }
        if reach < 1.0 - 1e-7 {
            break;
        }
        covered = w[1];
    }
    if axis == 1 && covered < 1.0 {
        return 0.0;
    }
    covered as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aabb(min: [f32; 3], max: [f32; 3]) -> Aabb {
        Aabb { min, max }
    }

    #[test]
    fn coverage_of_full_and_partial_faces() {
        let block = [aabb([0.0; 3], [1.0; 3])];
        assert_eq!((0..6).map(|f| coverage(&block, f)).collect::<Vec<_>>(), [1.0; 6]);
        assert_eq!(coverage(&[], 0), 0.0);
        let slab = [aabb([0.0; 3], [1.0, 0.5, 1.0])];
        assert_eq!((coverage(&slab, 4), coverage(&slab, 3), coverage(&slab, 2)), (0.5, 1.0, 0.0));
        // Stairs facing east: the bottom half and the east half on top of it.
        let stairs = [aabb([0.0; 3], [1.0, 0.5, 1.0]), aabb([0.5, 0.5, 0.0], [1.0, 1.0, 1.0])];
        assert_eq!((coverage(&stairs, 0), coverage(&stairs, 1), coverage(&stairs, 5)), (1.0, 0.5, 0.5));
        // A fence's 1.5-high post covers no side whole.
        let fence = [aabb([0.375, 0.0, 0.375], [0.625, 1.5, 0.625])];
        assert_eq!((coverage(&fence, 2), coverage(&fence, 4)), (0.0, 0.0));
        assert!(legacy_solid(&fence) && legacy_solid(&slab) && !legacy_solid(&[aabb([0.0; 3], [1.0, 0.125, 1.0])]));
    }
}
