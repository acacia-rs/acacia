//! Building chunks without a payload to decode (servers, tests): start from [`Chunk::empty`], then
//! [`Chunk::set`] single blocks or write whole sections and biomes here.

use super::storage::{Storage, VOLUME, index};
use super::{Chunk, Section};

impl Chunk {
    /// Makes section `section_y` (world y >> 4) one block throughout, without liquid. Out-of-range
    /// sections are ignored, as in [`Chunk::set`].
    pub fn fill_section(&mut self, section_y: i32, id: u32) {
        let section = Section { blocks: Storage::Single(id), liquid: Storage::Single(self.dim.air) };
        self.put_section(section_y - (self.dim.min_y >> 4), section);
    }

    /// Replaces section `section_y` with runtime ids in XZY order, `(x << 8) | (z << 4) | y`;
    /// no `liquid` means none.
    pub fn set_section(&mut self, section_y: i32, blocks: &[u32; VOLUME], liquid: Option<&[u32; VOLUME]>) {
        let liquid = liquid.map_or(Storage::Single(self.dim.air), Storage::from_values);
        let section = Section { blocks: Storage::from_values(blocks), liquid };
        self.put_section(section_y - (self.dim.min_y >> 4), section);
    }

    /// Gives every section of the column one biome.
    pub fn fill_biomes(&mut self, id: u32) {
        self.biomes = vec![Storage::Single(id); self.dim.sections()];
    }

    /// Sets the biome at a world position. Sections never given biomes take those of the section
    /// below (biome 0 at the bottom), which is also how they encode.
    pub fn set_biome(&mut self, x: i32, y: i32, z: i32, id: u32) {
        let Some(slot) = self.slot(y) else { return };
        while self.biomes.len() <= slot {
            self.biomes.push(self.biomes.last().cloned().unwrap_or(Storage::Single(0)));
        }
        self.biomes[slot].set(index(x, y, z), id);
    }
}
