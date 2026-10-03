//! Render shapes. Vanilla block shapes are hard-coded in the game, not in the resource pack, so
//! solid blocks use their collision boxes and collisionless ones get a small hand-written table.

use acacia_world::BlockState;

/// Box in 1/16 block units: min xyz, max xyz.
pub type Box16 = [u8; 6];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shape {
    None,
    Cube,
    Boxes(Box<[Box16]>),
    /// Two crossed planes (flowers, grass, crops).
    Cross,
    /// Water or lava; the surface height comes from the state.
    Liquid,
}

const INVISIBLE: &[&str] = &[
    "air", "light_block", "structure_void", "barrier", "bubble_column", "tripwire", "lever", "end_portal", "portal",
    "frame", "moving_block", "piston_arm_collision", "sticky_piston_arm_collision",
];
/// Rendered as nothing for now: they need block-entity data or attachment faces.
const UNSUPPORTED: &[&str] = &["sign", "banner", "vine", "glow_lichen", "sculk_vein", "resin_clump", "skull", "head"];

pub fn classify(state: &BlockState) -> Shape {
    let name = short_name(state.name);
    if state.is_air() || INVISIBLE.iter().any(|n| name == *n || name.starts_with("light_block")) {
        return Shape::None;
    }
    if state.is_liquid() {
        return Shape::Liquid;
    }
    if state.is_full_cube() {
        return Shape::Cube;
    }
    if !state.boxes.is_empty() {
        return Shape::Boxes(state.boxes.iter().map(|b| to_box16(b.min, b.max)).collect());
    }
    if UNSUPPORTED.iter().any(|n| name.contains(n)) {
        return Shape::None;
    }
    let flat = |h: u8| Shape::Boxes(Box::new([[0, 0, 0, 16, h, 16]]));
    match name {
        n if n.contains("carpet") || n.contains("rail") || n == "redstone_wire" || n == "moss_carpet" => flat(1),
        n if n.contains("pressure_plate") => Shape::Boxes(Box::new([[1, 0, 1, 15, 1, 15]])),
        n if n.contains("torch") => Shape::Boxes(Box::new([[7, 0, 7, 9, 10, 9]])),
        n if n.contains("button") => Shape::Boxes(Box::new([[5, 0, 6, 11, 2, 10]])),
        _ => Shape::Cross,
    }
}

pub fn short_name(name: &str) -> &str {
    name.strip_prefix("minecraft:").unwrap_or(name)
}

/// Rounds to 1/16 and clamps to the block (fence and wall collision reaches 1.5 high).
fn to_box16(min: [f32; 3], max: [f32; 3]) -> Box16 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 16.0).round() as u8;
    [q(min[0]), q(min[1]), q(min[2]), q(max[0]), q(max[1]), q(max[2])]
}
