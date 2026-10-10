//! Render shapes. Vanilla block shapes are hard-coded in the game, not in the resource pack, so
//! solid blocks use their collision boxes and collisionless ones get a small hand-written table.

use std::sync::Arc;

use acacia_world::BlockState;
use serde::{Deserialize, Serialize};

use super::{Material, Tint};

/// Box in 1/16 block units: min xyz, max xyz.
pub type Box16 = [u8; 6];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    None,
    Cube,
    Boxes(Box<[Box16]>),
    /// Two crossed planes (flowers, grass, crops).
    Cross,
    /// Water or lava; the surface height comes from the state.
    Liquid,
    /// A baked model: faces that carry their own texture, tint and material.
    Model(Arc<[ModelFace]>),
}

/// One face of a [`Shape::Model`], a parallelogram.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelFace {
    /// Corners 0, 1 and 3 in 1/16 block, counter-clockwise seen from the front; the block spans
    /// 0..16 and a face may reach a block beyond it.
    pub corners: [[f32; 3]; 3],
    /// Texels at those corners.
    pub uv: [[f32; 2]; 3],
    pub texture: u16,
    pub tint: Tint,
    pub material: Material,
    /// The axis face it is shaded and lit as ([`crate::assets::FACE_NAMES`] order); `None` takes
    /// no directional shade.
    pub shade: Option<u8>,
    /// Darkens towards occluding neighbours; only a shaded face does.
    pub ambient_occlusion: bool,
    /// Hidden when the neighbour on this side occludes.
    pub cull: Option<u8>,
}

const INVISIBLE: &[&str] = &[
    "air", "light_block", "structure_void", "barrier", "bubble_column", "tripwire",
    "frame", "glow_frame", "moving_block", "piston_arm_collision", "sticky_piston_arm_collision",
];
/// Rendered as nothing for now: it needs a model of its own.
const UNSUPPORTED: &[&str] = &["piglin_head"];
/// Blocks that cling to the faces `multi_face_direction_bits` names.
const SPREADING: &[&str] = &["glow_lichen", "sculk_vein", "resin_clump"];

/// Sheets without thickness a pixel off the faces a block clings to: down, up, north, south,
/// west, east.
fn sheets(on: [bool; 6]) -> Shape {
    const SHEETS: [Box16; 6] = [[0, 1, 0, 16, 1, 16], [0, 15, 0, 16, 15, 16], [0, 0, 1, 16, 16, 1], [0, 0, 15, 16, 16, 15], [1, 0, 0, 1, 16, 16], [15, 0, 0, 15, 16, 16]];
    Shape::Boxes(SHEETS.into_iter().zip(on).filter_map(|(sheet, on)| on.then_some(sheet)).collect())
}

/// A lever's plate against the block it is on and its handle standing out of it (not thrown
/// either way). `lever_direction` names the face the handle points out of.
fn lever(direction: &str) -> [Box16; 2] {
    match direction {
        "east" => [[0, 4, 5, 3, 12, 11], [3, 7, 7, 13, 9, 9]],
        "west" => [[13, 4, 5, 16, 12, 11], [3, 7, 7, 13, 9, 9]],
        "south" => [[5, 4, 0, 11, 12, 3], [7, 7, 3, 9, 9, 13]],
        "north" => [[5, 4, 13, 11, 12, 16], [7, 7, 3, 9, 9, 13]],
        d if d.starts_with("down") => [[5, 13, 4, 11, 16, 12], [7, 3, 7, 9, 13, 9]],
        _ => [[5, 0, 4, 11, 3, 12], [7, 3, 7, 9, 13, 9]],
    }
}

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
    let bits = |key: &str| state.property(key).and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);
    let bit = |bits: u8, n: u8| bits >> n & 1 == 1;
    match name {
        // South, west, north, east; none of them hangs it under the block above.
        "vine" => {
            let b = bits("vine_direction_bits");
            sheets([false, b == 0, bit(b, 2), bit(b, 0), bit(b, 1), bit(b, 3)])
        }
        // Down, up, south, west, north, east.
        n if SPREADING.contains(&n) => {
            let b = bits("multi_face_direction_bits");
            sheets([bit(b, 0), bit(b, 1), bit(b, 4), bit(b, 2), bit(b, 3), bit(b, 5)])
        }
        "lever" => Shape::Boxes(Box::new(lever(state.property("lever_direction").unwrap_or("up_north_south")))),
        "powder_snow" | "end_gateway" => Shape::Cube,
        // Sheets without thickness: edges would show as seams between a portal's blocks.
        "portal" if state.property("portal_axis") == Some("z") => Shape::Boxes(Box::new([[8, 0, 0, 8, 16, 16]])),
        "portal" => Shape::Boxes(Box::new([[0, 0, 8, 16, 16, 8]])),
        "end_portal" => Shape::Boxes(Box::new([[0, 12, 0, 16, 12, 16]])),
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
    assert_eq!(shape("portal", &["portal_axis=z"]), Shape::Boxes(Box::new([[8, 0, 0, 8, 16, 16]])));
    assert_eq!(shape("portal", &["portal_axis=x"]), Shape::Boxes(Box::new([[0, 0, 8, 16, 16, 8]])));
    assert_eq!(shape("end_portal", &[]), Shape::Boxes(Box::new([[0, 12, 0, 16, 12, 16]])));
    // A vine on the south and east faces; lichen on the floor and the north face.
    assert_eq!(shape("vine", &["vine_direction_bits=9"]), Shape::Boxes(Box::new([[0, 0, 15, 16, 16, 15], [15, 0, 0, 15, 16, 16]])));
    assert_eq!(shape("glow_lichen", &["multi_face_direction_bits=17"]), Shape::Boxes(Box::new([[0, 1, 0, 16, 1, 16], [0, 0, 1, 16, 16, 1]])));
    assert_eq!(shape("lever", &["lever_direction=up_north_south", "open_bit=0"]), Shape::Boxes(Box::new([[5, 0, 4, 11, 3, 12], [7, 3, 7, 9, 13, 9]])));
}

/// Rounds to 1/16 and clamps to the block (fence and wall collision reaches 1.5 high).
fn to_box16(min: [f32; 3], max: [f32; 3]) -> Box16 {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 16.0).round() as u8;
    [q(min[0]), q(min[1]), q(min[2]), q(max[0]), q(max[1]), q(max[2])]
}
