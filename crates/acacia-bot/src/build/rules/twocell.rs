//! Blocks one placement fills two cells: doors, beds, tall plants. Measured with `place_probe SET=twocell`
//! (2026-10-28, BDS 1.26.52.3); the table is in docs/builder/twocell.md. The planner side is `build/pairs.rs`.

use acacia_physics::{BlockPos, Vec3};
use acacia_world::BlockRegistry;

use super::{look, Dir};
use crate::build::structure::{Block, BED_COLOR};
use crate::interact::Face;
use crate::pathfind::process::Look;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Door,
    Bed,
    Plant,
}

/// Small dripleaf only stands on clay or moss on dry land (grass refused); it faces the player.
const PLANTS: &[&str] = &["sunflower", "lilac", "rose_bush", "peony", "tall_grass", "large_fern", "pitcher_plant", "small_dripleaf_block"];

pub fn kind(name: &str) -> Option<Kind> {
    let n = name.trim_start_matches("minecraft:");
    match n {
        "bed" => Some(Kind::Bed),
        _ if n.ends_with("_door") && !n.ends_with("trapdoor") => Some(Kind::Door),
        _ if PLANTS.contains(&n) => Some(Kind::Plant),
        _ => None,
    }
}

/// The legacy `direction` (beds): south 0, west 1, north 2, east 3.
fn legacy(d: Dir) -> &'static str {
    match d {
        Dir::South => "0",
        Dir::West => "1",
        Dir::North => "2",
        _ => "3",
    }
}

fn from_legacy(v: &str) -> Option<Dir> {
    Some([Dir::South, Dir::West, Dir::North, Dir::East][v.parse::<usize>().ok().filter(|i| *i < 4)?])
}

fn from_word(v: &str) -> Option<Dir> {
    Some(match v {
        "north" => Dir::North,
        "south" => Dir::South,
        "west" => Dir::West,
        "east" => Dir::East,
        _ => return None,
    })
}

fn clockwise(d: Dir) -> Dir {
    match d {
        Dir::North => Dir::East,
        Dir::East => Dir::South,
        Dir::South => Dir::West,
        _ => Dir::North,
    }
}

fn step(p: BlockPos, d: Dir) -> BlockPos {
    let o = match d {
        Dir::North => [0, 0, -1],
        Dir::South => [0, 0, 1],
        Dir::West => [-1, 0, 0],
        Dir::East => [1, 0, 0],
        Dir::Up => [0, 1, 0],
        Dir::Down => [0, -1, 0],
    };
    [p[0] + o[0], p[1] + o[1], p[2] + o[2]]
}

fn bit(b: &Block, k: &str) -> bool {
    b.prop(k).or_else(|| b.prop(&format!("minecraft:{k}"))) == Some("1")
}

/// The other cell of a two-cell target at `pos`, and whether `pos` is the first (the cell placed from).
pub fn partner(target: &Block, pos: BlockPos) -> Option<(BlockPos, bool)> {
    match kind(&target.name)? {
        Kind::Door | Kind::Plant if bit(target, "upper_block_bit") => Some((step(pos, Dir::Down), false)),
        Kind::Door | Kind::Plant => Some((step(pos, Dir::Up), true)),
        Kind::Bed => {
            let d = from_legacy(target.prop("direction")?)?;
            if bit(target, "head_piece_bit") { Some((step(pos, d.opposite()), false)) } else { Some((step(pos, d), true)) }
        }
    }
}

/// A mirrored door after the plain mirror of its states: its facing is the look turned clockwise, so it also
/// turns half round, and its hinge swaps sides.
pub fn mirror_door(b: Block) -> Block {
    if kind(&b.name) != Some(Kind::Door) {
        return b;
    }
    let props: Vec<String> = b
        .props
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| match (k.trim_start_matches("minecraft:"), from_word(v)) {
            ("cardinal_direction", Some(d)) => format!("{k}={}", d.opposite().word()),
            ("door_hinge_bit", _) => format!("{k}={}", if v == "1" { "0" } else { "1" }),
            _ => format!("{k}={v}"),
        })
        .collect();
    Block::new(&b.name, &props.join(","))
}

/// Keys of two-cell blocks the placement (or the pair) decides.
pub fn controlled(name: &str, k: &str) -> bool {
    kind(name).is_some() && matches!(k, "upper_block_bit" | "door_hinge_bit" | "open_bit" | "head_piece_bit" | "occupied_bit" | "cardinal_direction" | "direction" | BED_COLOR)
}

/// A state no placement gives: an open door (the interact tier), an occupied bed.
pub fn unplaceable(target: &Block) -> bool {
    kind(&target.name).is_some() && bit(target, "occupied_bit")
}

/// Not compared: BDS keeps a door's hinge in its upper half and writes 0 in the lower.
pub fn ignored(target: &Block, k: &str) -> bool {
    k == "door_hinge_bit" && kind(&target.name) == Some(Kind::Door) && !bit(target, "upper_block_bit")
}

/// Placing the first cell of a two-cell target by this click gives its states: doors and beds only from the Up
/// face below (side clicks were refused), plants from the side too; a door faces the look turned clockwise, a
/// bed the look (its head the cell ahead), small dripleaf the player. `None`: not a two-cell block.
pub fn gives(target: &Block, face: Face, eye: Vec3, point: Vec3) -> Option<bool> {
    let k = kind(&target.name)?;
    match Dir::of(face) {
        Dir::Up => {}
        Dir::Down => return Some(false),
        _ if k != Kind::Plant => return Some(false),
        _ => {}
    }
    let looks = look(eye, point);
    let facing = target.prop("minecraft:cardinal_direction").or(target.prop("cardinal_direction"));
    Some(match k {
        Kind::Door => looks.map(clockwise).map(Dir::word) == facing,
        Kind::Bed => looks.map(legacy) == target.prop("direction"),
        Kind::Plant => facing.is_none_or(|f| looks.map(|d| d.opposite().word()) == Some(f)),
    })
}

/// The hinge BDS would give a door placed now with the upper half `upper` at `pos`, from its neighbours (left
/// and right as seen from where it is placed): 1 with a door left and none right, or more opaque full blocks in
/// the two cells right than left (glass, slabs, fences and doors don't count); `None`: not a door.
pub fn hinge_now(upper: &Block, pos: BlockPos, look: &dyn Look) -> Option<&'static str> {
    hinge_with(upper, pos, &|p| look.block(p).map(|v| (v.name.to_string(), v.props.to_string())))
}

/// [`hinge_now`] with the neighbours `block` gives (name, props; `None`: air or unloaded).
pub fn hinge_with(upper: &Block, pos: BlockPos, block: &dyn Fn(BlockPos) -> Option<(String, String)>) -> Option<&'static str> {
    if kind(&upper.name) != Some(Kind::Door) {
        return None;
    }
    let (left, right) = sides(upper, pos)?;
    let door = |p: BlockPos| block(p).is_some_and(|(n, s)| kind(&n) == Some(Kind::Door) && s.contains("upper_block_bit=0"));
    let r = BlockRegistry::vanilla();
    let opaque = |p: BlockPos| block(p).and_then(|(n, s)| r.find(&n, &s)).and_then(|id| r.get(id)).is_some_and(|s| s.is_full_cube() && s.light_filter >= 15);
    let count = |p: BlockPos| [p, step(p, Dir::Up)].into_iter().filter(|c| opaque(*c)).count();
    Some(if (door(left) && !door(right)) || count(right) > count(left) { "1" } else { "0" })
}

/// The lower cells left and right of the door whose upper half is at `pos`, as seen from the placing side.
pub fn sides(upper: &Block, pos: BlockPos) -> Option<(BlockPos, BlockPos)> {
    let right = from_word(upper.prop("minecraft:cardinal_direction").or(upper.prop("cardinal_direction"))?)?;
    let lower = step(pos, Dir::Down);
    Some((step(lower, right.opposite()), step(lower, right)))
}

/// The bed colour the target asks for ([`BED_COLOR`]).
pub fn color(target: &Block) -> Option<u8> {
    target.prop(BED_COLOR)?.parse().ok()
}

/// A coloured bed's item key: `bed` with the colour as aux.
pub fn item(target: &Block) -> Option<String> {
    Some(crate::pathfind::process::item_key::join(&target.name, Some(u32::from(color(target)?))))
}
