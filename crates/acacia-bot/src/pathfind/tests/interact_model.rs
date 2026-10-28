//! The interact tier's blocks as the simulations' server, from the `place_probe SET=interact` and
//! `interact_probe` tables (2026-10-28), apart from `build/rules/interact.rs` so the two can disagree.

use acacia_physics::{BlockPos, Vec3};
use acacia_world::BlockAccess;

use super::build_sim::{take, World};
use crate::interact::{block_distance, Face, BLOCK_REACH};
use crate::pathfind::process::{Act, BlockView};
use crate::ActionError;

/// A use-click or sign edit from `eye`, as the server answers it.
pub fn act(w: &mut World, eye: Vec3, act: &Act) -> Result<(), ActionError> {
    let (Act::Use { pos, .. } | Act::Sign { pos, .. }) = act else { unreachable!("uses and signs only") };
    let cell = w.g.registry.get(w.g.view.block(pos[0], pos[1], pos[2])).map(BlockView::of).expect("loaded");
    if block_distance(eye, *pos) > BLOCK_REACH {
        return Err(ActionError::NotPossible(format!("{pos:?} out of reach")));
    }
    match act {
        Act::Use { item, .. } => {
            let held = item.as_deref().filter(|i| w.inv.get(*i).copied().unwrap_or(0) > 0);
            let (name, props) = use_block(&cell, held).ok_or_else(|| ActionError::Rejected(format!("using {pos:?} changed nothing")))?;
            if held == Some(cell.name) {
                take(w, cell.name)?;
            }
            w.set(*pos, name.trim_start_matches("minecraft:"), &props);
        }
        Act::Sign { front, text, .. } => {
            // The editor the server opened on placing is the front, wherever the player stands.
            if !(*front || sign_side(&cell, *pos, eye) == Some(false)) {
                return Err(ActionError::NotPossible(format!("the server opened the other side of {pos:?}")));
            }
            w.signs.insert((*pos, *front), text.clone());
        }
        _ => {}
    }
    Ok(())
}

const DIRS: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

fn looks(eye: Vec3, point: Vec3) -> usize {
    let (dx, dz) = (point[0] - eye[0], point[2] - eye[2]);
    if dx.abs() > dz.abs() { if dx > 0.0 { 5 } else { 4 } } else if dz > 0.0 { 3 } else { 2 }
}

fn opposite(d: usize) -> usize {
    d ^ 1
}

fn count(cell: &BlockView, key: &str) -> Option<usize> {
    let v = cell.prop(key)?;
    ["0", "1", "2", "3"].iter().position(|c| *c == v).or_else(|| ["one_egg", "two_egg", "three_egg", "four_egg"].iter().position(|c| *c == v))
}

/// The placed state (name without prefix, props) for this tier's items; `Some(None)`: refused; `None`: not one.
pub fn place(item: &str, face: Face, eye: Vec3, point: Vec3, cell: &BlockView) -> Option<Option<(String, String)>> {
    let n = item.trim_start_matches("minecraft:");
    let (f, l) = (face as usize, looks(eye, point));
    let toward = DIRS[opposite(l)];
    let axis = |d: usize| if d < 4 { "north_south" } else { "east_west" };
    let held = cell.solid();
    let state = |name: &str, props: String| Some(Some((name.to_string(), props)));
    match n {
        _ if held && n.ends_with("candle") && cell.name.trim_start_matches("minecraft:") == n => match count(cell, "candles") {
            Some(c) if c < 3 => state(n, format!("candles={},lit=0", c + 1)),
            _ => Some(None),
        },
        _ if held => None,
        "repeater" => state("unpowered_repeater", format!("minecraft:cardinal_direction={toward},repeater_delay=0")),
        "comparator" => state("unpowered_comparator", format!("minecraft:cardinal_direction={toward},output_lit_bit=0,output_subtract_bit=0")),
        "daylight_detector" => state(n, "redstone_signal=0".into()),
        "lever" => state(n, format!("lever_direction={},open_bit=0", match face {
            Face::Up => format!("up_{}", axis(l)),
            Face::Down => format!("down_{}", axis(l)),
            _ => DIRS[f].to_string(),
        })),
        "oak_sign" => match face {
            Face::Down => Some(None),
            Face::Up => {
                let yaw = (-(point[0] - eye[0])).atan2(point[2] - eye[2]).to_degrees();
                state("standing_sign", format!("ground_sign_direction={}", ((yaw + 180.0) / 22.5).round().rem_euclid(16.0) as u32))
            }
            _ => state("wall_sign", format!("facing_direction={f}")),
        },
        "oak_hanging_sign" => {
            // Side faces: towards the player along the wall (interact_probe and the live sign scene, 2026-10-28).
            let facing = match face {
                Face::Up | Face::Down => opposite(l),
                _ if f < 4 => if eye[0] > point[0] { 5 } else { 4 },
                _ => if eye[2] > point[2] { 3 } else { 2 },
            };
            state(n, format!("attached_bit=0,facing_direction={facing},ground_sign_direction=0,hanging={}", u8::from(face == Face::Down)))
        }
        _ if n.ends_with("candle") => state(n, "candles=0,lit=0".into()),
        "sea_pickle" => state(n, "cluster_count=0,dead_bit=1".into()),
        "turtle_egg" => state(n, "cracked_state=no_cracks,turtle_egg_count=one_egg".into()),
        _ => None,
    }
}

/// The cell after a right-click holding `held`; `None`: nothing changes.
pub fn use_block(cell: &BlockView, held: Option<&str>) -> Option<(String, String)> {
    let n = cell.name.trim_start_matches("minecraft:");
    let set = |key: &str, value: &str| {
        let props: Vec<String> = cell.props.split(',').map(|kv| match kv.split_once('=') {
            Some((k, _)) if k == key => format!("{k}={value}"),
            _ => kv.to_string(),
        }).collect();
        Some((n.to_string(), props.join(",")))
    };
    let flip = |key: &str| set(key, if cell.prop(key) == Some("1") { "0" } else { "1" });
    match n {
        "unpowered_repeater" | "powered_repeater" => set("repeater_delay", &((count(cell, "repeater_delay")? + 1) % 4).to_string()),
        "unpowered_comparator" | "powered_comparator" => flip("output_subtract_bit"),
        "daylight_detector" => Some(("daylight_detector_inverted".into(), cell.props.into())),
        "daylight_detector_inverted" => Some(("daylight_detector".into(), cell.props.into())),
        _ if n.starts_with("iron_") => None,
        _ if cell.prop("open_bit").is_some() && (n == "lever" || n.contains("door") || n.contains("fence_gate")) => flip("open_bit"),
        "sea_pickle" if held == Some(cell.name) => count(cell, "cluster_count").filter(|c| *c < 3).and_then(|c| set("cluster_count", &(c + 1).to_string())),
        "turtle_egg" if held == Some(cell.name) => {
            let c = count(cell, "turtle_egg_count").filter(|c| *c < 3)?;
            set("turtle_egg_count", ["one_egg", "two_egg", "three_egg", "four_egg"][c + 1])
        }
        _ => None,
    }
}

/// The side of the sign at `pos` the server opens for a player whose eye is at `eye` (front: `true`).
pub fn sign_side(cell: &BlockView, pos: BlockPos, eye: Vec3) -> Option<bool> {
    let normal = match (cell.prop("ground_sign_direction"), cell.prop("facing_direction")) {
        (Some(r), _) if cell.name.ends_with("standing_sign") => {
            let a = (r.parse::<f32>().ok()? * 22.5).to_radians();
            [-a.sin(), a.cos()]
        }
        (_, Some(f)) => match f {
            "2" => [0.0, -1.0],
            "3" => [0.0, 1.0],
            "4" => [-1.0, 0.0],
            _ => [1.0, 0.0],
        },
        _ => return None,
    };
    Some((eye[0] - pos[0] as f32 - 0.5) * normal[0] + (eye[2] - pos[2] as f32 - 0.5) * normal[1] > 0.0)
}
