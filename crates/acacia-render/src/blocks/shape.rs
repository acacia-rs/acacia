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
        "powder_snow" | "end_gateway" => Shape::Cube,
        "snow_layer" => flat(2),
        n if FLAT.contains(&n) || n.contains("carpet") || n.contains("rail") => flat(1),
        n if n.ends_with("fence_gate") => Shape::Boxes(Box::new(open_gate(state))),
        n if n.contains("pressure_plate") => Shape::Boxes(Box::new([[1, 0, 1, 15, 1, 15]])),
        n if n.contains("torch") => Shape::Boxes(Box::new([[7, 0, 7, 9, 10, 9]])),
        n if n.contains("button") => Shape::Boxes(Box::new([[5, 0, 6, 11, 2, 10]])),
        _ => Shape::Cross,
    }
}

const FLAT: &[&str] = &["redstone_wire", "leaf_litter", "pink_petals", "wildflowers", "frog_spawn"];

/// Closed gates have collision and take that path; an open one is two posts with the leaves swung
/// towards its facing.
fn open_gate(state: &BlockState) -> [Box16; 4] {
    let facing = state.property("minecraft:cardinal_direction").unwrap_or("south");
    let (from, to) = if matches!(facing, "south" | "east") { (9, 15) } else { (1, 7) };
    let along_x = [[0, 5, 7, 2, 16, 9], [14, 5, 7, 16, 16, 9], [0, 6, from, 2, 15, to], [14, 6, from, 16, 15, to]];
    if matches!(facing, "south" | "north") { along_x } else { along_x.map(|[x0, y0, z0, x1, y1, z1]| [z0, y0, x0, z1, y1, x1]) }
}

pub fn short_name(name: &str) -> &str {
    name.strip_prefix("minecraft:").unwrap_or(name)
}

#[test]
fn collisionless_blocks_that_are_not_plants_get_boxes() {
    let reg = acacia_world::BlockRegistry::vanilla();
    let shape = |name: &str, props: &[&str]| {
        let state = (0..reg.len() as u32).filter_map(|id| reg.get(id));
        let found = state.filter(|s| short_name(s.name) == name).find(|s| props.iter().all(|p| s.properties.contains(p)));
        classify(found.unwrap_or_else(|| panic!("no state {name} {props:?}")))
    };
    let east_gate = shape("fence_gate",&["open_bit=1", "cardinal_direction=east"]);
    assert_eq!(east_gate, Shape::Boxes(Box::new([[7, 5, 0, 9, 16, 2], [7, 5, 14, 9, 16, 16], [9, 6, 0, 15, 15, 2], [9, 6, 14, 15, 15, 16]])));
    assert_eq!(shape("snow_layer", &["height=0"]), Shape::Boxes(Box::new([[0, 0, 0, 16, 2, 16]])));
    assert_eq!(shape("powder_snow", &[]), Shape::Cube);
    assert_eq!(shape("poppy", &[]), Shape::Cross);
}

/// Rounds to 1/16 and clamps to the block (fence and wall collision reaches 1.5 high).
fn to_box16(min: [f32; 3], max: [f32; 3]) -> Box16 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 16.0).round() as u8;
    [q(min[0]), q(min[1]), q(min[2]), q(max[0]), q(max[1]), q(max[2])]
}
