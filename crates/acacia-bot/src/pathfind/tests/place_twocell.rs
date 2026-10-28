//! How BDS places blocks that fill two cells (doors, beds, tall plants), as the simulations' server: written
//! from the place_probe `SET=twocell` table (2026-10-28), apart from `build/rules/twocell.rs` so the two can
//! disagree.

use acacia_physics::{BlockPos, Vec3};
use acacia_world::BlockState;

use crate::interact::Face;

const PLANTS: &[&str] = &["sunflower", "lilac", "rose_bush", "peony", "tall_grass", "large_fern", "pitcher_plant", "small_dripleaf_block"];
/// North, east, south, west, as cardinal words and bed `direction`s.
const WORDS: [&str; 4] = ["north", "east", "south", "west"];
const LEGACY: [&str; 4] = ["2", "3", "0", "1"];
const STEP: [[i32; 3]; 4] = [[0, 0, -1], [1, 0, 0], [0, 0, 1], [-1, 0, 0]];

fn at(p: BlockPos, d: [i32; 3]) -> BlockPos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

fn is_door(name: &str) -> bool {
    name.ends_with("_door") && !name.ends_with("trapdoor")
}

/// The cells and states BDS sets for `item` clicked on `face` at `point` from `eye`, filling `pos` first;
/// `None`: not a two-cell item; `Some(None)`: refused. `block` reads the world.
pub fn bds_pair<'a>(item: &str, face: Face, eye: Vec3, point: Vec3, pos: BlockPos, block: &dyn Fn(BlockPos) -> Option<&'a BlockState>) -> Option<Option<Vec<(BlockPos, String, String)>>> {
    let n = item.trim_start_matches("minecraft:");
    let (door, bed, plant) = (is_door(n), n == "bed", PLANTS.contains(&n));
    if !(door || bed || plant) {
        return None;
    }
    let empty = |p: BlockPos| block(p).is_some_and(|s| s.is_air());
    let (dx, dz) = (point[0] - eye[0], point[2] - eye[2]);
    let look = if dx.abs() > dz.abs() { if dx > 0.0 { 1 } else { 3 } } else if dz > 0.0 { 2 } else { 0 };
    let up = at(pos, [0, 1, 0]);
    let refused = (face == Face::Down) || (!plant && face != Face::Up);
    Some(match () {
        _ if refused => None,
        _ if bed => {
            let head = at(pos, STEP[look]);
            empty(head).then(|| {
                let s = |h: u8| format!("direction={},head_piece_bit={h},occupied_bit=0", LEGACY[look]);
                vec![(pos, "bed".into(), s(0)), (head, "bed".into(), s(1))]
            })
        }
        _ if !empty(up) => None,
        _ if plant => {
            let name = n.to_string();
            let facing = if n == "small_dripleaf_block" { format!("minecraft:cardinal_direction={},", WORDS[(look + 2) % 4]) } else { String::new() };
            Some(vec![(pos, name.clone(), format!("{facing}upper_block_bit=0")), (up, name, format!("{facing}upper_block_bit=1"))])
        }
        _ => {
            let facing = (look + 1) % 4;
            let (left, right) = (at(pos, STEP[(facing + 2) % 4]), at(pos, STEP[facing]));
            let door_at = |p: BlockPos| block(p).is_some_and(|s| is_door(s.name) && s.properties.contains("upper_block_bit=0"));
            // Probe: stone counts, glass, slabs, fences and doors don't.
            let solid = |p: BlockPos| [p, at(p, [0, 1, 0])].iter().filter(|c| block(**c).is_some_and(|s| s.is_full_cube() && s.light_filter == 15)).count();
            let hinge = u8::from((door_at(left) && !door_at(right)) || solid(right) > solid(left));
            let s = |h: u8, u: u8| format!("door_hinge_bit={h},minecraft:cardinal_direction={},open_bit=0,upper_block_bit={u}", WORDS[facing]);
            let name = n.to_string();
            Some(vec![(pos, name.clone(), s(0, 0)), (up, name, s(hinge, 1))])
        }
    })
}

/// The other half of the two-cell block `s` at `pos`: BDS breaks both.
pub fn other_half(s: &BlockState, pos: BlockPos) -> Option<BlockPos> {
    let n = s.name.trim_start_matches("minecraft:");
    let upper = s.properties.contains("upper_block_bit=1");
    match () {
        _ if is_door(n) || PLANTS.contains(&n) => Some(at(pos, [0, if upper { -1 } else { 1 }, 0])),
        _ if n == "bed" => {
            let d = s.properties.split(',').find_map(|kv| kv.strip_prefix("direction="))?;
            let i = LEGACY.iter().position(|l| *l == d)?;
            let i = if s.properties.contains("head_piece_bit=1") { (i + 2) % 4 } else { i };
            Some(at(pos, STEP[i]))
        }
        _ => None,
    }
}
