//! What a block is to light in Java ([`LightFacts`]): rules over the Bedrock state it maps from, the
//! Java state and its baked model, checked against the game by `lookbake java-light-check`
//! (tools/parity harness `light_facts`).

use acacia_render::blocks::{FaceShapes, LightFacts, ModelFace, RenderBlock, Shape};
use acacia_world::BlockState;

use crate::mapping::JavaState;
use crate::shade;

/// Blocks the light engine sees by their shape (`useShapeForLightOcclusion`), but for double slabs.
const SHAPED: &[&str] = &[
    "daylight_detector", "enchanting_table", "end_portal_frame", "farmland", "lectern", "dirt_path", "sculk_sensor",
    "calibrated_sculk_sensor", "sculk_shrieker", "snow", "stonecutter",
];
/// Drawn at full brightness whatever the light (`emissiveRendering`).
const EMISSIVE: &[&str] = &["magma_block"];
/// Full blocks that let sky light straight down (`TransparentBlock` and kin) and blocks that
/// never do, whatever their shape.
const SKY_THROUGH: &[&str] = &["glass", "barrier", "light"];
const SKY_STOPPED: &[&str] = &["tinted_glass", "chorus_plant", "shulker_box"];
const FLUIDS: &[&str] = &["water", "lava", "bubble_column", "kelp", "kelp_plant", "seagrass", "tall_seagrass"];

/// `light_shape` interns face shapes as [`LightFacts::shape`].
pub fn facts(java: &JavaState, bedrock: &BlockState, block: &RenderBlock, light_shape: impl FnOnce(FaceShapes) -> u16) -> LightFacts {
    let name = java.name.as_str();
    let has = |names: &[&str]| names.iter().any(|n| name == *n || name.ends_with(&format!("_{n}")));
    let full_collision = shade::full_collision(java, bedrock.is_full_cube());
    let solid_render = block.occludes && block.shape == Shape::Cube;
    let fluid = java.property("waterlogged") == Some("true") || FLUIDS.contains(&name);
    let sky_through = !has(SKY_STOPPED) && (has(SKY_THROUGH) || (!bedrock.is_full_cube() && !fluid));
    let dampening = match () {
        _ if name.ends_with("leaves") => 1,
        _ if solid_render || name == "tinted_glass" => 15,
        _ if sky_through => 0,
        _ => 1,
    };
    let emission = bedrock.light_emission;
    let shaped = (has(SHAPED) || name.ends_with("_slab") && java.property("type") != Some("double") || name.ends_with("_stairs") || name.ends_with("_shelf"))
        && block.shape != Shape::None;
    LightFacts {
        emission,
        dampening,
        solid_render,
        full_collision,
        emissive: EMISSIVE.contains(&name),
        dark: shade::darkens(java, bedrock.is_full_cube()),
        ambient_occlusion: block.light.ambient_occlusion && emission == 0,
        shape: if shaped { light_shape(face_shapes(block)) } else { 0 },
    }
}

/// The block's faces flush with each side, as [`FaceShapes`] grids.
fn face_shapes(block: &RenderBlock) -> FaceShapes {
    let Shape::Model(faces) = &block.shape else { return [[u64::MAX; 4]; 6] };
    let mut out = [[0u64; 4]; 6];
    for f in faces.iter() {
        if let Some((d, lo, hi)) = flush(f) {
            for v in 0..16 {
                for u in 0..16 {
                    let (cu, cv) = (u as f32 + 0.5, v as f32 + 0.5);
                    if cu > lo[0] && cu < hi[0] && cv > lo[1] && cv < hi[1] {
                        let bit = v * 16 + u;
                        out[d][bit / 64] |= 1 << (bit % 64);
                    }
                }
            }
        }
    }
    out
}

/// The Java direction of the side a face lies on, and its bounds on that side's (u, v) grid in
/// 1/16 block: (x, z) on y sides, (x, y) on z sides, (z, y) on x sides.
fn flush(f: &ModelFace) -> Option<(usize, [f32; 2], [f32; 2])> {
    let [c0, c1, c3] = f.corners;
    let c2 = [0, 1, 2].map(|i| c1[i] + c3[i] - c0[i]);
    let corners = [c0, c1, c2, c3];
    let (axis, at) = (0..3).find_map(|axis| {
        let v = corners[0][axis];
        (corners.iter().all(|c| (c[axis] - v).abs() < 1e-4) && (v.abs() < 1e-4 || (v - 16.0).abs() < 1e-4)).then_some((axis, v))
    })?;
    let (u_axis, v_axis) = match axis {
        0 => (2, 1),
        1 => (0, 2),
        _ => (0, 1),
    };
    let bounds = |a: usize| corners.iter().map(|c| c[a]).fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
    let ((u0, u1), (v0, v1)) = (bounds(u_axis), bounds(v_axis));
    // DOWN, UP, NORTH, SOUTH, WEST, EAST.
    let d = match (axis, at > 8.0) {
        (1, false) => 0,
        (1, true) => 1,
        (2, false) => 2,
        (2, true) => 3,
        (0, false) => 4,
        _ => 5,
    };
    Some((d, [u0, v0], [u1, v1]))
}
