//! Per-Java-state light and heightmap data, approximating Java's from what Bedrock's registry carries.
//!
//! Opacity follows Java's `getLightBlock`: 15 for opaque blocks, 1 for blocks that hold a fluid or are
//! translucent but full (leaves, ice, slime), 0 otherwise. Slabs, stairs and similar block light only
//! through their full faces (Java's `useShapeForLightOcclusion`), from `faces`.

use acacia_world::{BlockFlags, BlockRegistry};

use crate::mapping::{self, JavaState, StateMap};

pub const DOWN: usize = 0;
pub const UP: usize = 1;
pub const NORTH: usize = 2;
pub const SOUTH: usize = 3;
pub const WEST: usize = 4;
pub const EAST: usize = 5;

pub const fn opposite(d: usize) -> usize {
    d ^ 1
}

pub const NON_AIR: u8 = 1;
/// Java `blocksMotion` (OCEAN_FLOOR, MOTION_BLOCKING heightmaps).
pub const BLOCKS_MOTION: u8 = 2;
pub const FLUID: u8 = 4;
pub const LEAVES: u8 = 8;

#[derive(Clone, Copy, Default)]
pub struct StateProps {
    pub opacity: u8,
    pub emission: u8,
    /// Bit per direction: that face is full for light occlusion.
    pub faces: u8,
    pub flags: u8,
}

pub struct PropsTable(Vec<StateProps>);

impl PropsTable {
    pub fn new(registry: &BlockRegistry, map: &StateMap) -> Self {
        let j = mapping::java();
        let table = (0..j.state_count())
            .map(|s| {
                let state = s as JavaState;
                let name = j.block(state).name;
                let Some(b) = registry.get(map.bedrock(state)) else {
                    return StateProps::default();
                };
                let wet = j.property(state, "waterlogged") == Some("true")
                    || b.is_liquid()
                    || always_wet(name);
                let faces = occlusion_faces(state);
                let mut opacity = match b.light_filter {
                    0 => 0,
                    15 => 15,
                    _ if faces != 0 => 0,
                    _ => 1,
                };
                if wet {
                    opacity = opacity.max(1);
                }
                let mut flags = 0;
                if !b.flags.contains(BlockFlags::AIR) && name != "minecraft:air" {
                    flags |= NON_AIR;
                }
                if b.is_solid() && !b.flags.contains(BlockFlags::COBWEB) {
                    flags |= BLOCKS_MOTION;
                }
                if wet {
                    flags |= FLUID;
                }
                if name.ends_with("_leaves") {
                    flags |= LEAVES;
                }
                StateProps {
                    opacity,
                    emission: b.light_emission,
                    faces,
                    flags,
                }
            })
            .collect();
        Self(table)
    }

    #[inline]
    pub fn get(&self, s: JavaState) -> StateProps {
        self.0[s as usize]
    }
}

fn always_wet(name: &str) -> bool {
    matches!(
        name,
        "minecraft:kelp"
            | "minecraft:kelp_plant"
            | "minecraft:seagrass"
            | "minecraft:tall_seagrass"
            | "minecraft:bubble_column"
    )
}

fn dir_of(facing: &str) -> Option<usize> {
    Some(match facing {
        "north" => NORTH,
        "south" => SOUTH,
        "west" => WEST,
        "east" => EAST,
        _ => return None,
    })
}

fn clockwise(d: usize) -> usize {
    match d {
        NORTH => EAST,
        EAST => SOUTH,
        SOUTH => WEST,
        _ => NORTH,
    }
}

/// Full faces of the blocks whose light occlusion uses their shape.
fn occlusion_faces(state: JavaState) -> u8 {
    let j = mapping::java();
    let name = j.block(state).name;
    let prop = |k| j.property(state, k);
    if name.ends_with("_slab") {
        return match prop("type") {
            Some("bottom") => 1 << DOWN,
            Some("top") => 1 << UP,
            _ => 0x3F,
        };
    }
    if name.ends_with("_stairs") {
        let vertical = if prop("half") == Some("top") {
            1 << UP
        } else {
            1 << DOWN
        };
        let Some(f) = prop("facing").and_then(dir_of) else {
            return vertical;
        };
        let side = match prop("shape") {
            Some("straight") => 1 << f,
            Some("inner_left") => (1 << f) | (1 << opposite(clockwise(f))),
            Some("inner_right") => (1 << f) | (1 << clockwise(f)),
            _ => 0,
        };
        return vertical | side;
    }
    match name {
        "minecraft:farmland" | "minecraft:dirt_path" | "minecraft:snow" => 1 << DOWN,
        _ => 0,
    }
}
