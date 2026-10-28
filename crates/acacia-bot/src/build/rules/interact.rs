//! States that take more than a placement click (measured with `examples/place_probe.rs SET=interact` and
//! `examples/interact_probe.rs`, BDS 1.26.52.3, 2026-10-28): use-click toggles, extra placements into a held
//! cell for counts, levers, signs. Table and evidence: docs/builder/interact.md.

use acacia_physics::Vec3;

use super::{key, Dir};
use crate::build::structure::Block;
use crate::pathfind::process::BlockView;

fn bare(name: &str) -> &str {
    name.trim_start_matches("minecraft:")
}

/// Never compared: redstone output and light (`output_lit_bit`, `redstone_signal`), out of water
/// (`dead_bit`), hatching (`cracked_state`).
pub(super) fn derived(k: &str) -> bool {
    matches!(k, "output_lit_bit" | "redstone_signal" | "dead_bit" | "cracked_state")
}

/// The item that places the block, where it is not the block's own name.
pub(super) fn item(name: &str) -> Option<String> {
    let n = bare(name);
    let item = match n {
        "unpowered_repeater" | "powered_repeater" => "repeater".to_string(),
        "unpowered_comparator" | "powered_comparator" => "comparator".to_string(),
        "daylight_detector_inverted" => "daylight_detector".to_string(),
        "standing_sign" | "wall_sign" => "oak_sign".to_string(),
        _ => {
            let wood = n.strip_suffix("_standing_sign").or_else(|| n.strip_suffix("_wall_sign"))?;
            format!("{}_sign", if wood == "darkoak" { "dark_oak" } else { wood })
        }
    };
    Some(format!("minecraft:{item}"))
}

/// Powered and unpowered repeaters and comparators are one block: power is derived.
pub(super) fn same(target: &str, seen: &str) -> bool {
    matches!(bare(target), "unpowered_repeater" | "powered_repeater" | "unpowered_comparator" | "powered_comparator") && item(target) == item(seen)
}

/// Clicking opens and closes it by hand (iron doors and trapdoors need redstone; a barrel's `open_bit` is its
/// lid while someone looks inside).
fn opens(name: &str) -> bool {
    let n = bare(name);
    (n.contains("door") || n.contains("fence_gate") || n == "lever") && !n.starts_with("iron_")
}

/// The count state of blocks a placement into their own cell adds to: key, and the count a value means.
fn count_of(name: &str, k: &str, v: &str) -> Option<u32> {
    let n = bare(name);
    match k {
        "candles" if n.ends_with("candle") => v.parse::<u32>().ok().map(|c| c + 1),
        "cluster_count" if n == "sea_pickle" => v.parse::<u32>().ok().map(|c| c + 1),
        "turtle_egg_count" if n == "turtle_egg" => ["one_egg", "two_egg", "three_egg", "four_egg"].iter().position(|e| *e == v).map(|i| i as u32 + 1),
        _ => None,
    }
}

fn props(b: &Block) -> impl Iterator<Item = (&str, &str)> {
    b.props.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (key(k), v))
}

/// Placements the target takes in its cell (a count state), if it has one.
pub(super) fn count(target: &Block) -> Option<u32> {
    props(target).find_map(|(k, v)| count_of(&target.name, k, v))
}

/// Count still to add when the cell holds the target's block with a smaller count.
fn missing(target: &Block, w: &BlockView) -> Option<u32> {
    let have = w.props.split(',').filter_map(|kv| kv.split_once('=')).find_map(|(k, v)| count_of(w.name, key(k), v))?;
    (target.name == w.name).then(|| count(target)?.checked_sub(have)).flatten().filter(|n| *n > 0)
}

/// Candles are added by a placement into their cell (against the floor, as slabs are made double); sea pickles
/// and turtle eggs refuse that and take a click on the block itself (`uses`).
fn added_by_use(name: &str) -> bool {
    matches!(bare(name), "sea_pickle" | "turtle_egg")
}

/// The cell holds fewer candles than the target: one more placement into it.
pub(super) fn adds(target: &Block, w: &BlockView) -> bool {
    !added_by_use(&target.name) && missing(target, w).is_some()
}

/// Bedrock keeps a door's upper half at `open_bit=0` whether open or not (copper door, 2026-10-28).
pub(super) fn ignored(target: &Block, k: &str) -> bool {
    let n = bare(&target.name);
    k == "open_bit" && n.ends_with("door") && !n.ends_with("trapdoor") && props(target).any(|(k, v)| k == "upper_block_bit" && v == "1")
}

fn sign(name: &str) -> bool {
    bare(name).ends_with("_sign")
}

/// The key, with this value, is one the builder gets right: by the click, by use-clicks, or by placements.
pub(super) fn known(name: &str, k: &str, v: &str) -> bool {
    let n = bare(name);
    match k {
        "open_bit" => opens(name),
        "repeater_delay" | "output_subtract_bit" | "lever_direction" => true,
        "ground_sign_direction" => n.ends_with("standing_sign") || (n.ends_with("hanging_sign") && v == "0"),
        "facing_direction" | "hanging" => sign(name),
        // Attached (chain-style) hanging signs come from sneaking or a thin block above: not measured.
        "attached_bit" => n.ends_with("hanging_sign") && v == "0",
        // Lighting needs flint and steel: lit candles stay unsupported.
        "lit" => n.ends_with("candle") && v == "0",
        _ => count_of(name, k, v).is_some(),
    }
}

/// Bedrock yaw (degrees) of looking from `eye` at `point`.
fn yaw(eye: Vec3, point: Vec3) -> f32 {
    (-(point[0] - eye[0])).atan2(point[2] - eye[2]).to_degrees()
}

/// Sixteenths of a turn from south: `round((yaw + 180) / 22.5) mod 16`; `None` within 4 degrees of a boundary.
fn rotation(eye: Vec3, point: Vec3) -> Option<u32> {
    let r = (yaw(eye, point) + 180.0).rem_euclid(360.0) / 22.5;
    ((r - r.floor() - 0.5).abs() >= 4.0 / 22.5).then(|| (r.round() as u32) % 16)
}

/// Whether a click on `dir`'s face at `point` from `eye` gives the key's value; `None`: not a key of this tier.
pub(super) fn gives(name: &str, k: &str, v: &str, dir: Dir, look: Option<Dir>, eye: Vec3, point: Vec3) -> Option<bool> {
    let n = bare(name);
    let z = |d: Dir| matches!(d, Dir::North | Dir::South);
    let axis = |d: Dir| if z(d) { "north_south" } else { "east_west" };
    let hanging = n.ends_with("hanging_sign");
    let want = match k {
        "lever_direction" => match dir {
            Dir::Up => look.map(|d| format!("up_{}", axis(d))),
            Dir::Down => look.map(|d| format!("down_{}", axis(d))),
            d => Some(d.word().to_string()),
        },
        "ground_sign_direction" if n.ends_with("standing_sign") => (dir == Dir::Up).then(|| rotation(eye, point)).flatten().map(|r| r.to_string()),
        "facing_direction" if sign(n) && !hanging => (!matches!(dir, Dir::Up | Dir::Down)).then(|| dir.int().to_string()),
        // Towards the player; on a side face, along the wall (BDS breaks an exact tie its own way: avoided).
        "facing_direction" if hanging => match dir {
            Dir::Up | Dir::Down => look.map(|l| l.opposite().int().to_string()),
            d => {
                let along = if z(d) { eye[0] - point[0] } else { eye[2] - point[2] };
                let toward = match (z(d), along > 0.0) {
                    (true, true) => Dir::East,
                    (true, false) => Dir::West,
                    (false, true) => Dir::South,
                    (false, false) => Dir::North,
                };
                (along.abs() >= 0.2).then(|| toward.int().to_string())
            }
        },
        "hanging" if hanging => Some((if dir == Dir::Down { "1" } else { "0" }).to_string()),
        "open_bit" if opens(n) => return Some(true),
        "lit" if n.ends_with("candle") => return Some(true),
        "repeater_delay" | "output_subtract_bit" | "attached_bit" | "ground_sign_direction" => return Some(true),
        _ if count_of(name, k, v).is_some() => return Some(true),
        _ => return None,
    };
    Some(want.as_deref() == Some(v))
}

/// The eye is in front of the sign `w` at `pos` (`Some(false)`: behind it; `None`: too near its plane to tell).
/// The server opens the editor of the side the player stands on.
pub fn front(w: &BlockView, pos: [i32; 3], eye: Vec3) -> Option<bool> {
    let rotation = w.prop("ground_sign_direction").filter(|_| !bare(w.name).ends_with("hanging_sign") || w.prop("attached_bit") == Some("1"));
    let normal = match (rotation.and_then(|r| r.parse::<f32>().ok()), w.prop("facing_direction")) {
        (Some(r), _) => {
            let a = (r * 22.5).to_radians();
            [-a.sin(), a.cos()]
        }
        (None, Some("2")) => [0.0, -1.0],
        (None, Some("3")) => [0.0, 1.0],
        (None, Some("4")) => [-1.0, 0.0],
        (None, Some("5")) => [1.0, 0.0],
        _ => return None,
    };
    let d = (eye[0] - pos[0] as f32 - 0.5) * normal[0] + (eye[2] - pos[2] as f32 - 0.5) * normal[1];
    (d.abs() > 0.3).then_some(d > 0.0)
}

/// Use-clicks that turn the block in the cell into the target, if only use-toggled states differ (or a sea
/// pickle or turtle egg count), and the item to hold for them (`None`: any).
pub fn uses(target: &Block, w: &BlockView) -> Option<(u32, Option<String>)> {
    let item_now = super::item(target)?;
    if item_now != item(w.name).unwrap_or_else(|| w.name.to_string()) {
        return None;
    }
    if added_by_use(&target.name) {
        return missing(target, w).map(|n| (n, Some(item_now)));
    }
    // A different block of the same item: only the daylight detector turns into its inverted self by a click.
    let renamed = target.name != w.name && !same(&target.name, w.name);
    if renamed && !bare(&target.name).starts_with("daylight_detector") {
        return None;
    }
    let mut clicks = u32::from(renamed);
    for (k, v) in props(target) {
        let Some(have) = w.prop(k).or_else(|| w.prop(&format!("minecraft:{k}"))) else { continue };
        if have == v || super::derived(k) || ignored(target, k) {
            continue;
        }
        clicks += match k {
            "open_bit" if opens(&target.name) => 1,
            "output_subtract_bit" => 1,
            "repeater_delay" => (v.parse::<u32>().ok()? + 4 - have.parse::<u32>().ok()?) % 4,
            _ => return None,
        };
    }
    (clicks > 0).then_some((clicks, None))
}
