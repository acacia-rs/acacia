//! How strict BDS sets a placed block's state, measured with `examples/place_probe.rs` (2026-10-17, BDS
//! 1.26.52.3): which states the click decides, which the neighbours do, and which blocks the builder places.

use acacia_physics::{BlockPos, Vec3};

use super::structure::Block;
use crate::interact::Face;
use crate::pathfind::process::BlockView;

mod interact;
mod oneclick;
pub mod twocell;
pub use interact::{fine_aim, front, uses};

/// Recomputed from the neighbours by the server (stair corners, wall/fence/pane connections), or redstone
/// activity: never compared.
fn derived(key: &str) -> bool {
    key == "corner" || key.starts_with("wall_connection_type_") || key == "wall_post_bit" || key.starts_with("connection_") || matches!(key, "powered_bit" | "triggered_bit" | "button_pressed_bit") || oneclick::derived(key) || interact::derived(key)
}

/// Outside the common building set (B1): their states take more than one click, or they are not blocks one places.
/// `candle_cake`: a candle clicked onto a cake, not placed.
const DENIED: &[&str] = &[
    "bedrock", "redstone", "water", "lava", "rail", "banner", "skull", "head", "portal", "fire", "sapling", "wheat", "carrots",
    "potatoes", "beetroot", "vine", "kelp", "seagrass", "bubble", "piston_arm", "moving_block", "bamboo", "cactus", "sugar_cane", "candle_cake", "scaffolding",
];

/// Facing kinds of `facing_direction` blocks.
#[derive(Clone, Copy, PartialEq)]
enum Facing {
    /// The clicked face (ladders: side faces only).
    Clicked,
    /// Six-way from the player: (horizontal, vertical), each away from the player (`true`) or towards it.
    Player(bool, bool),
}

fn facing_kind(name: &str) -> Option<Facing> {
    let n = name.trim_start_matches("minecraft:");
    match n {
        "ladder" => Some(Facing::Clicked),
        _ if n.ends_with("_button") => Some(Facing::Clicked),
        "observer" => Some(Facing::Player(true, true)),
        "piston" | "sticky_piston" => Some(Facing::Player(true, false)),
        "dispenser" | "dropper" => Some(Facing::Player(false, false)),
        _ => None,
    }
}

fn key(k: &str) -> &str {
    k.trim_start_matches("minecraft:")
}

fn is_double_slab(name: &str) -> bool {
    name.ends_with("_double_slab")
}

/// Every state of the target is one a click decides or the server derives.
pub fn supported(name: &str) -> bool {
    let n = name.trim_start_matches("minecraft:");
    oneclick::claims(n) || !DENIED.iter().any(|d| n.contains(d))
}

fn controlled(name: &str, k: &str) -> bool {
    if oneclick::controls(name, k) {
        return true;
    }
    match k {
        "pillar_axis" | "vertical_half" | "upside_down_bit" | "weirdo_direction" | "cardinal_direction" => true,
        "torch_facing_direction" => true,
        "facing_direction" => facing_kind(name).is_some(),
        _ => twocell::controlled(name, k),
    }
}

/// The target's keys are all known: decided by the click, derived, or absent.
fn keys_known(target: &Block) -> bool {
    target.props.split(',').filter_map(|kv| kv.split_once('=')).all(|(k, v)| {
        interact::known(&target.name, key(k), v)
            || ((derived(key(k)) || controlled(&target.name, key(k)) || (is_double_slab(&target.name) && key(k) == "vertical_half")) && oneclick::fixed_ok(key(k), v))
    })
}

/// The item that places the target.
pub fn item(target: &Block) -> Option<String> {
    if !supported(&target.name) || !keys_known(target) || twocell::unplaceable(target) {
        return None;
    }
    Some(twocell::item(target).or_else(|| interact::item(&target.name)).unwrap_or_else(|| oneclick::base(&target.name).replace("_double_slab", "_slab")))
}

/// Placements into the cell past its first that add to it (candles); slabs keep counting as placements.
pub fn more_placements(target: &Block) -> u32 {
    interact::count(target).map_or(0, |c| c.saturating_sub(1))
}

pub fn items_per_cell(target: &Block) -> u32 {
    if is_double_slab(&target.name) { 2 } else { interact::count(target).unwrap_or(1) }
}

/// A block a body fits beside within its cell (panes, bars, fences, walls), so a player can end up inside the
/// cell it fills; see `SearchOpts::no_body`.
pub fn thin(name: &str) -> bool {
    ["pane", "iron_bars", "fence", "_wall"].iter().any(|k| name.contains(k)) && !name.contains("gate")
}

pub fn gravity(name: &str) -> bool {
    let n = name.trim_start_matches("minecraft:");
    matches!(n, "sand" | "red_sand" | "gravel" | "suspicious_sand" | "suspicious_gravel") || n.ends_with("concrete_powder") || oneclick::gravity(n)
}

/// The target can hang from the block above, so that block may cover it while it is open: `Some(true)` if it
/// only hangs, `Some(false)` if it can also be placed onto the block under it.
pub fn hangs(target: &Block) -> Option<bool> {
    oneclick::hangs(&target.name, &target.props)
}

/// A partial block whose Up and Down faces take a click at their centre.
pub fn post(b: &BlockView) -> bool {
    oneclick::post(b.name, b.props)
}

/// The world shows the target: same block, same states except derived ones (and a double slab's half).
pub fn matches(target: &Block, w: &BlockView) -> bool {
    if oneclick::base(&target.name) != oneclick::base(w.name) && !interact::same(&target.name, w.name) {
        return false;
    }
    target.props.split(',').filter_map(|kv| kv.split_once('=')).all(|(k, v)| {
        let k2 = key(k);
        derived(k2)
            || twocell::ignored(target, k2)
            || interact::ignored(target, k2)
            || (is_double_slab(&target.name) && k2 == "vertical_half")
            || w.prop(k).or_else(|| w.prop(k2)).is_none_or(|wv| wv == v || oneclick::alike(k2, v, wv))
    })
}

/// The cell holds the half slab that one more slab makes into the target, or fewer candles (pickles, eggs).
pub fn completes(target: &Block, w: &BlockView) -> bool {
    (is_double_slab(&target.name) && item(target).is_some_and(|i| i == w.name)) || interact::adds(target, w)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Dir {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Dir {
    fn of(face: Face) -> Dir {
        [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East][face as usize]
    }

    fn opposite(self) -> Dir {
        match self {
            Dir::Down => Dir::Up,
            Dir::Up => Dir::Down,
            Dir::North => Dir::South,
            Dir::South => Dir::North,
            Dir::West => Dir::East,
            Dir::East => Dir::West,
        }
    }

    fn int(self) -> &'static str {
        ["0", "1", "2", "3", "4", "5"][self as usize]
    }

    fn word(self) -> &'static str {
        ["down", "up", "north", "south", "west", "east"][self as usize]
    }

    fn weirdo(self) -> &'static str {
        match self {
            Dir::East => "0",
            Dir::West => "1",
            Dir::South => "2",
            _ => "3",
        }
    }
}

/// The horizontal direction the player looks from `eye` at `point`; `None` too near a diagonal to be sure of.
fn look(eye: Vec3, point: Vec3) -> Option<Dir> {
    let (dx, dz) = (point[0] - eye[0], point[2] - eye[2]);
    let (ax, az) = (dx.abs(), dz.abs());
    if ax.max(az) < 1.2 * ax.min(az) {
        return None;
    }
    Some(if ax > az { if dx > 0.0 { Dir::East } else { Dir::West } } else if dz > 0.0 { Dir::South } else { Dir::North })
}

/// Six-way facing, towards the player: up or down with the eye over 2 above the block's bottom or below it,
/// within a block horizontally (measured vertical at 1.0, horizontal at 1.9: between is avoided); else the
/// horizontal look. `None` near a boundary.
fn six_way(eye: Vec3, pos: BlockPos, look: Option<Dir>) -> Option<Dir> {
    let above = eye[1] - pos[1] as f32;
    if (above - 2.0).abs() <= 0.1 || above.abs() <= 0.1 {
        return None;
    }
    let horizontal = look.map(Dir::opposite);
    if (0.0..2.0).contains(&above) {
        return horizontal;
    }
    let off = (eye[0] - pos[0] as f32 - 0.5).abs().max((eye[2] - pos[2] as f32 - 0.5).abs());
    match off {
        _ if off < 1.05 => Some(if above > 0.0 { Dir::Up } else { Dir::Down }),
        _ if off >= 1.9 => horizontal,
        _ => None,
    }
}

/// Placing the target's item by clicking `face` of `against` at `click` from `eye` gives the target's
/// click-decided states (into `pos`).
pub fn gives(target: &Block, face: Face, click: Vec3, eye: Vec3, pos: BlockPos, _against: &BlockView) -> bool {
    if is_double_slab(&target.name) {
        return true;
    }
    let dir = Dir::of(face);
    let point = [(pos[0] - face.offset()[0]) as f32 + click[0], (pos[1] - face.offset()[1]) as f32 + click[1], (pos[2] - face.offset()[2]) as f32 + click[2]];
    if let Some(g) = twocell::gives(target, face, eye, point) {
        return g;
    }
    let top = match dir {
        Dir::Down => true,
        Dir::Up => false,
        _ => click[1] > 0.5,
    };
    let name = target.name.as_str();
    let mine = oneclick::Click { dir, look: look(eye, point), toward: six_way(eye, pos, look(eye, point)) };
    target.props.split(',').filter_map(|kv| kv.split_once('=')).all(|(k, v)| {
        // Interact first: its use-set states (open_bit) would otherwise fail oneclick's placement-fixed values.
        if let Some(ok) = interact::gives(name, key(k), v, dir, look(eye, point), eye, point) {
            return ok;
        }
        if let Some(want) = oneclick::decide(name, key(k), &mine, v) {
            return want == Some(v);
        }
        let want = match key(k) {
            "pillar_axis" => Some(match dir {
                Dir::Up | Dir::Down => "y",
                Dir::North | Dir::South => "z",
                _ => "x",
            }),
            "vertical_half" => Some(if top { "top" } else { "bottom" }),
            "upside_down_bit" => Some(if top { "1" } else { "0" }),
            "weirdo_direction" => look(eye, point).map(Dir::weirdo),
            "cardinal_direction" => look(eye, point).map(|d| d.opposite().word()),
            "torch_facing_direction" => match dir {
                Dir::Up => Some("top"),
                Dir::Down => None,
                d => Some(d.opposite().word()),
            },
            "facing_direction" => match facing_kind(name) {
                Some(Facing::Clicked) if name.ends_with("ladder") && matches!(dir, Dir::Up | Dir::Down) => None,
                Some(Facing::Clicked) => Some(if v.parse::<u8>().is_ok() { dir.int() } else { dir.word() }),
                Some(Facing::Player(away_h, away_v)) => six_way(eye, pos, look(eye, point))
                    .map(|d| match d {
                        Dir::Up | Dir::Down if away_v => d.opposite(),
                        Dir::Up | Dir::Down => d,
                        _ if away_h => d.opposite(),
                        _ => d,
                    })
                    .map(|d| if v.parse::<u8>().is_ok() { d.int() } else { d.word() }),
                None => None,
            },
            k if derived(k) => return true,
            _ => return true,
        };
        want == Some(v)
    })
}
