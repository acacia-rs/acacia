//! Light levels and per-block light properties of the loaded columns, shared between the light
//! thread (writer) and the mesh workers (readers, [`LightData::gather`]).

use rustc_hash::FxHashSet;

use super::sections::Sections;
use crate::mesh::volume::{SIDE, cell};
use crate::workers::SectionKey;

/// Light byte: block light << 4 | sky light.
pub const fn pack(block: u8, sky: u8) -> u8 {
    block << 4 | sky
}

/// Props byte: emission << 4 | filter ([`acacia_world::BlockState::light_filter`]).
pub const fn props(emission: u8, filter: u8) -> u8 {
    emission << 4 | filter
}

pub const OPAQUE_FILTER: u8 = 15;
/// Cells of [`LightVolume`] that block all light; the shader leaves them out of smooth-light
/// averages. A real 15/15 cell is stored as 15/14 to keep the value free.
pub const OPAQUE_CELL: u8 = 0xFF;

/// A section's light with a one-block border, in [`cell`] order, as uploaded to the GPU.
pub struct LightVolume {
    /// [`LightData::generation`] when gathered; newer volumes replace older ones.
    pub generation: u64,
    pub cells: Box<[u8; SIDE * SIDE * SIDE]>,
}

#[derive(Default)]
pub struct LightData {
    pub light: Sections,
    pub props: Sections,
    /// Lit columns.
    pub columns: FxHashSet<(i32, i32)>,
    pub generation: u64,
    /// Sections whose [`LightVolume`] changed since the last [`LightData::take_touched`].
    touched: FxHashSet<SectionKey>,
}

impl LightData {
    #[inline]
    pub fn light(&self, x: i32, y: i32, z: i32) -> Option<u8> {
        self.light.get(x, y, z)
    }

    /// Sets a light byte and records every section whose bordered volume holds the block.
    #[inline]
    pub fn set_light(&mut self, x: i32, y: i32, z: i32, value: u8) {
        self.light.set(x, y, z, value);
        let near = |v: i32| match v & 15 {
            0 => -1..=0,
            15 => 0..=1,
            _ => 0..=0,
        };
        let (sx, sy, sz) = (x >> 4, y >> 4, z >> 4);
        for dx in near(x) {
            for dy in near(y) {
                for dz in near(z) {
                    self.touched.insert((sx + dx, sy + dy, sz + dz));
                }
            }
        }
    }

    pub fn take_touched(&mut self) -> FxHashSet<SectionKey> {
        std::mem::take(&mut self.touched)
    }

    pub fn gather(&self, (sx, sy, sz): SectionKey) -> LightVolume {
        let mut cells = Box::new([0u8; SIDE * SIDE * SIDE]);
        let (ox, oy, oz) = (sx * 16, sy * 16, sz * 16);
        for x in -1..=16 {
            for z in -1..=16 {
                for y in -1..=16 {
                    let (wx, wy, wz) = (ox + x, oy + y, oz + z);
                    let opaque = self.props.get(wx, wy, wz).is_some_and(|p| p & 15 == OPAQUE_FILTER);
                    cells[cell(x, y, z)] = if opaque {
                        OPAQUE_CELL
                    } else {
                        self.light.get(wx, wy, wz).map_or(0, |l| l.min(OPAQUE_CELL - 1))
                    };
                }
            }
        }
        LightVolume { generation: self.generation, cells }
    }
}
