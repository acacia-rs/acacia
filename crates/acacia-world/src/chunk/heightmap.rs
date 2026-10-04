//! The heightmap a `SubChunk` entry carries, after dragonfly (MIT) `server/session/chunk.go`: per
//! column the y above the highest counted block, relative to the sub-chunk. Unverified against a
//! vanilla client; see the crate README.

use acacia_proto::types::{HeightMap, HeightMapDataType};

use super::storage::VOLUME;
use super::Chunk;

/// Per column, the y just above the highest counted layer-0 block; `min_y` where there is none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnHeights([i32; 256]);

/// One sub-chunk's share of a [`ColumnHeights`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heightmap {
    /// Every column tops out above this sub-chunk.
    TooHigh,
    /// Every column tops out below this sub-chunk.
    TooLow,
    /// Index `(z << 4) | x`: 0..=15 inside the sub-chunk, 16 above it, -1 below it.
    Data([i8; 256]),
}

impl Chunk {
    /// `counts` picks the runtime ids that hold the heightmap up (dragonfly: those that filter light).
    pub fn column_heights(&self, counts: &dyn Fn(u32) -> bool) -> ColumnHeights {
        let min_y = self.dim.min_y;
        let mut heights = [min_y; 256];
        let mut buf = [0; VOLUME];
        for (slot, section) in self.sections.iter().enumerate().rev() {
            let Some(section) = section else { continue };
            if !section.blocks.palette().iter().any(|&id| counts(id)) {
                continue;
            }
            section.blocks.copy_into(&mut buf);
            let above = min_y + slot as i32 * 16 + 1;
            for (column, height) in heights.iter_mut().enumerate().filter(|(_, h)| **h == min_y) {
                let (x, z) = (column & 15, column >> 4);
                let at = (x << 8) | (z << 4);
                if let Some(y) = (0..16).rev().find(|y| counts(buf[at | y])) {
                    *height = above + y as i32;
                }
            }
        }
        ColumnHeights(heights)
    }
}

impl ColumnHeights {
    pub fn get(&self, x: i32, z: i32) -> i32 {
        self.0[(((z & 15) << 4) | (x & 15)) as usize]
    }

    /// The heightmap of section `section_y` (world y >> 4).
    pub fn sub_chunk(&self, section_y: i32) -> Heightmap {
        let section = |h: &i32| h >> 4;
        if self.0.iter().all(|h| section(h) > section_y) {
            return Heightmap::TooHigh;
        }
        if self.0.iter().all(|h| section(h) < section_y) {
            return Heightmap::TooLow;
        }
        Heightmap::Data(self.0.map(|h| match section(&h) - section_y {
            0 => (h & 15) as i8,
            d if d > 0 => 16,
            _ => -1,
        }))
    }
}

impl Heightmap {
    pub fn data_type(&self) -> HeightMapDataType {
        match self {
            Heightmap::TooHigh => HeightMapDataType::TooHigh,
            Heightmap::TooLow => HeightMapDataType::TooLow,
            Heightmap::Data(_) => HeightMapDataType::HasData,
        }
    }

    /// The packet field: 16 rows of 16, one row per z.
    pub fn rows(&self) -> Option<HeightMap> {
        let Heightmap::Data(data) = self else { return None };
        Some(std::array::from_fn(|z| data[z * 16..][..16].to_vec()))
    }
}
