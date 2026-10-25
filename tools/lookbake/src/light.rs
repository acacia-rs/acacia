//! What a block is to light in Java ([`LightFacts`]): rules over the Bedrock state it maps from, the
//! Java state and its baked model, checked against the game by `lookbake java-light-check`
//! (tools/parity harness `light_facts`).

use acacia_render::blocks::{FaceShapes, LightFacts, ModelFace, RenderBlock, Shape};
use acacia_world::BlockState;

use crate::mapping::JavaState;
use crate::shade;

/// Blocks the light engine sees by their shape (`useShapeForLightOcclusion`), but for slabs,
/// stairs and shelves.
const SHAPED: &[&str] = &[
    "daylight_detector", "enchanting_table", "end_portal_frame", "farmland", "lectern", "dirt_path", "sculk_sensor",
    "calibrated_sculk_sensor", "sculk_shrieker", "snow", "stonecutter", "piston_head",
];
/// Full blocks that let sky light straight down (`TransparentBlock` and kin), blocks that never
/// do whatever their shape, and both by name suffix.
const SKY_THROUGH: &[&str] = &["glass", "barrier", "light", "azalea", "dragon_egg", "stonecutter", "copper_grate"];
const SKY_STOPPED: &[&str] = &["tinted_glass", "chorus_plant", "shulker_box", "powder_snow", "honey_block", "cobweb", "sculk_shrieker", "end_gateway"];
/// Cubes the Bedrock look draws as occluders that Java does not render solid, and the reverse.
const NOT_SOLID: &[&str] = &["shulker_box", "end_gateway", "barrier"];
const SOLID: &[&str] = &["chiseled_bookshelf"];
const FLUIDS: &[&str] = &["water", "lava", "bubble_column", "kelp", "kelp_plant", "seagrass", "tall_seagrass"];
/// Light emission where Java's differs from Bedrock's by more than the state.
const EMISSION: &[(&str, u8)] = &[("crying_obsidian", 10), ("magma_block", 3)];

/// `light_shape` interns face shapes as [`LightFacts::shape`].
pub fn facts(java: &JavaState, bedrock: &BlockState, block: &RenderBlock, light_shape: impl FnOnce(FaceShapes) -> u16) -> LightFacts {
    let name = java.name.as_str();
    let has = |names: &[&str]| names.iter().any(|n| name == *n || name.ends_with(&format!("_{n}")));
    let full_collision = shade::full_collision(java, bedrock.is_full_cube());
    let solid_render = has(SOLID) || (block.occludes && block.shape == Shape::Cube && !has(NOT_SOLID));
    let fluid = java.property("waterlogged") == Some("true") || FLUIDS.contains(&name);
    // A face-less sculk vein or resin clump keeps the default rule though its shape is empty.
    let faceless = matches!(name, "sculk_vein" | "resin_clump") && ["down", "up", "north", "south", "west", "east"].iter().all(|f| java.property(f) == Some("false"));
    let sky_through = !has(SKY_STOPPED) && !faceless && (has(SKY_THROUGH) || (!bedrock.is_full_cube() && !fluid));
    let dampening = match () {
        _ if name.ends_with("leaves") => 1,
        _ if solid_render || name == "tinted_glass" => 15,
        _ if sky_through => 0,
        _ => 1,
    };
    let emission = emission(java, bedrock.light_emission);
    let shaped = (SHAPED.contains(&name) || name.ends_with("_slab") && java.property("type") != Some("double") || name.ends_with("_stairs") || name.ends_with("_shelf"))
        && block.shape != Shape::None;
    let emissive = name == "magma_block" || (name.ends_with("sculk_sensor") && java.property("sculk_sensor_phase") == Some("active"));
    LightFacts {
        emission,
        dampening,
        solid_render,
        full_collision,
        emissive,
        dark: shade::darkens(java, bedrock.is_full_cube()),
        ambient_occlusion: block.light.ambient_occlusion && emission == 0,
        shape: if shaped { light_shape(face_shapes(java, block)) } else { 0 },
    }
}

/// Java's `getLightEmission` of `java`, from Bedrock's where the rule is the same.
fn emission(java: &JavaState, bedrock: u8) -> u8 {
    let name = java.name.as_str();
    let any_face = || ["down", "up", "north", "south", "west", "east"].iter().any(|f| java.property(f) == Some("true"));
    match name {
        "sea_pickle" if java.property("waterlogged") != Some("true") => 0,
        "sea_pickle" => 3 + 3 * java.property("pickles").and_then(|p| p.parse::<u8>().ok()).unwrap_or(1),
        "glow_lichen" if !any_face() => 0,
        "trial_spawner" => match java.property("trial_spawner_state") {
            Some("waiting_for_players") => 4,
            Some("active" | "waiting_for_reward_ejection" | "ejecting_reward") => 8,
            _ => 0,
        },
        _ => EMISSION.iter().find(|(n, _)| *n == name).map_or(bedrock, |(_, e)| *e),
    }
}

/// The block's faces flush with each side, as [`FaceShapes`] grids.
fn face_shapes(java: &JavaState, block: &RenderBlock) -> FaceShapes {
    let Shape::Model(faces) = &block.shape else { return [[u64::MAX; 4]; 6] };
    let mut out = [[0u64; 4]; 6];
    // The piston arm crosses the side behind the head without a face on it.
    if java.name == "piston_head" {
        let facing = ["down", "up", "north", "south", "west", "east"].iter().position(|d| java.property("facing") == Some(d));
        if let Some(d) = facing {
            for bit in (6..10).flat_map(|v| (6..10).map(move |u| v * 16 + u)) {
                out[d ^ 1][bit / 64] |= 1 << (bit % 64);
            }
        }
    }
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
