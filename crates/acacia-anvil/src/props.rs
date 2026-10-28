//! Per-Java-state light and heightmap data, dumped from Java 26.3 itself (tools/javadump/LightDump.java):
//! `getLightDampening`, `getLightEmission`, the full faces of the light-occlusion shape of states that use
//! one (slabs, stairs, shelves, ...) and the heightmap tags.

use crate::mapping::{self, JavaState};

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
/// In `#blocks_motion` (OCEAN_FLOOR; with fluids, MOTION_BLOCKING).
pub const BLOCKS_MOTION: u8 = 2;
pub const FLUID: u8 = 4;
/// In `#blocks_motion` but not `#blocks_motion_no_leaves`.
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

impl Default for PropsTable {
    fn default() -> Self {
        Self::new()
    }
}

impl PropsTable {
    pub fn new() -> Self {
        let j = mapping::java();
        let table = (0..j.state_count())
            .map(|s| {
                let [opacity, emission, faces, flags] = j.light_data(s as JavaState);
                StateProps {
                    opacity,
                    emission,
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
