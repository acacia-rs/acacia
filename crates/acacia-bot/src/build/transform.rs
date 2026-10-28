//! Where a structure goes: local cells to world cells (mirror along x first, then a clockwise turn seen from
//! above), and the block states that name a direction or an axis turned with them.

use serde::{Deserialize, Serialize};

use super::structure::Block;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Transform {
    /// World cell of the structure's local (0, 0, 0) after turning: the minimum corner of the footprint.
    pub origin: [i32; 3],
    /// Clockwise quarter turns, 0..=3.
    pub turns: u8,
    /// Mirror along x (east and west swap) before turning.
    pub mirror: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    North,
    East,
    South,
    West,
}

impl Dir {
    const ALL: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

    fn turned(self, t: &Transform) -> Dir {
        let d = match (t.mirror, self) {
            (true, Dir::East) => Dir::West,
            (true, Dir::West) => Dir::East,
            (_, d) => d,
        };
        Dir::ALL[(d as usize + usize::from(t.turns % 4)) % 4]
    }
}

/// How each direction-carrying state names the four directions: (key, [north, east, south, west]).
const DIR_STATES: &[(&str, [&str; 4])] = &[
    ("cardinal_direction", ["north", "east", "south", "west"]),
    ("facing_direction", ["2", "5", "3", "4"]),
    ("weirdo_direction", ["3", "0", "2", "1"]),
    ("torch_facing_direction", ["north", "east", "south", "west"]),
    ("direction", ["2", "3", "0", "1"]),
    ("vine_direction_bits", ["4", "8", "1", "2"]),
];

impl Transform {
    /// World cell of local `p` in a structure of `size`.
    pub fn apply(&self, size: [i32; 3], [x, y, z]: [i32; 3]) -> [i32; 3] {
        let x = if self.mirror { size[0] - 1 - x } else { x };
        let (sx, sz) = (size[0], size[2]);
        let (x, z) = match self.turns % 4 {
            0 => (x, z),
            1 => (sz - 1 - z, x),
            2 => (sx - 1 - x, sz - 1 - z),
            _ => (z, sx - 1 - x),
        };
        [self.origin[0] + x, self.origin[1] + y, self.origin[2] + z]
    }

    /// The footprint's size in the world.
    pub fn world_size(&self, size: [i32; 3]) -> [i32; 3] {
        if self.turns % 2 == 1 { [size[2], size[1], size[0]] } else { size }
    }

    /// `b` with its direction and axis states turned like the cells.
    pub fn block(&self, b: &Block) -> Block {
        if self.turns % 4 == 0 && !self.mirror {
            return b.clone();
        }
        let props: Vec<String> = b
            .props
            .split(',')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| format!("{k}={}", self.value(k.trim_start_matches("minecraft:"), v)))
            .collect();
        Block::new(&b.name, &props.join(","))
    }

    fn value<'a>(&self, key: &str, v: &'a str) -> &'a str {
        if key == "rail_direction" {
            return super::rules::redstone::turned_rail(v, self.turns, self.mirror);
        }
        if key == "pillar_axis" && self.turns % 2 == 1 {
            return match v {
                "x" => "z",
                "z" => "x",
                v => v,
            };
        }
        let Some((_, names)) = DIR_STATES.iter().find(|(k, _)| *k == key) else { return v };
        match names.iter().position(|n| *n == v) {
            Some(i) => names[Dir::ALL[i].turned(self) as usize],
            None => v,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turns_cells_and_states_together() {
        let t = Transform { origin: [10, 0, 20], turns: 1, mirror: false };
        // A quarter turn clockwise: local +x (east) points south.
        assert_eq!(t.apply([4, 1, 2], [0, 0, 0]), [11, 0, 20]);
        assert_eq!(t.apply([4, 1, 2], [3, 0, 0]), [11, 0, 23]);
        let stairs = Block::new("oak_stairs", "upside_down_bit=0,weirdo_direction=0");
        assert_eq!(t.block(&stairs).prop("weirdo_direction"), Some("2"));
        assert_eq!(t.block(&Block::new("oak_log", "pillar_axis=x")).prop("pillar_axis"), Some("z"));
        let m = Transform { mirror: true, ..Transform::default() };
        assert_eq!(m.block(&Block::new("furnace", "minecraft:cardinal_direction=east")).prop("minecraft:cardinal_direction"), Some("west"));
        assert_eq!(m.block(&Block::new("ladder", "facing_direction=2")).prop("facing_direction"), Some("2"));
    }
}
