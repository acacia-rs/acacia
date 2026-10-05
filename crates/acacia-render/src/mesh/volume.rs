//! A section plus a one-block border copied out of the world, so meshing runs without locks.

use acacia_world::{SECTION_VOLUME, World};

pub const SIDE: usize = 18;
const CELLS: usize = SIDE * SIDE * SIDE;
/// Stands in for blocks of unloaded neighbour columns, sections not yet received, and below the
/// world: hides faces towards them, so an edge isn't drawn until its neighbour arrives (which
/// remeshes it).
pub const OCCLUDER: u32 = u32::MAX;

/// Biome id of cells whose biomes the server hasn't sent; tints like plains.
pub const NO_BIOME: u32 = u32::MAX;

pub struct Volume {
    /// World position of the section's first block.
    pub origin: [i32; 3],
    pub blocks: Box<[u32; CELLS]>,
    pub liquid: Box<[u32; CELLS]>,
    /// Reaches [`BLEND`] columns past the section, by [`biome_cell`].
    pub biomes: Box<[u32; BIOME_CELLS]>,
}

/// Cell index for section-local coordinates in -1..=16.
#[inline]
pub fn cell(x: i32, y: i32, z: i32) -> usize {
    (((x + 1) as usize * SIDE) + (z + 1) as usize) * SIDE + (y + 1) as usize
}

/// The widest biome blend ([`crate::look::Look::biome_blend`]).
pub const BLEND: i32 = 2;
const BIOME_SIDE: usize = 16 + 2 * BLEND as usize;
const BIOME_CELLS: usize = BIOME_SIDE * BIOME_SIDE * SIDE;

/// Biome cell index for x and z in -BLEND..16 + BLEND, y in -1..=16.
#[inline]
pub(super) fn biome_cell(x: i32, y: i32, z: i32) -> usize {
    (((x + BLEND) as usize * BIOME_SIDE) + (z + BLEND) as usize) * SIDE + (y + 1) as usize
}

impl Volume {
    /// Every cell `block`, no liquid, biome 0.
    #[cfg(test)]
    pub fn filled(block: u32) -> Volume {
        Volume { origin: [0; 3], blocks: Box::new([block; CELLS]), liquid: Box::new([0; CELLS]), biomes: Box::new([0; BIOME_CELLS]) }
    }

    #[inline]
    pub fn block(&self, x: i32, y: i32, z: i32) -> u32 {
        self.blocks[cell(x, y, z)]
    }

    #[inline]
    pub fn liquid(&self, x: i32, y: i32, z: i32) -> u32 {
        self.liquid[cell(x, y, z)]
    }

    #[inline]
    pub fn biome(&self, x: i32, y: i32, z: i32) -> u32 {
        self.biomes[biome_cell(x, y, z)]
    }

    /// Copies section `section_y` (world y >> 4) of column `(cx, cz)` with its border. `None` when
    /// the column isn't loaded.
    pub fn gather(world: &World, cx: i32, section_y: i32, cz: i32) -> Option<Volume> {
        world.get(cx, cz)?;
        let dim = world.dimension();
        let mut v = Volume {
            origin: [cx * 16, section_y * 16, cz * 16],
            blocks: Box::new([OCCLUDER; CELLS]),
            liquid: Box::new([dim.air; CELLS]),
            biomes: Box::new([NO_BIOME; BIOME_CELLS]),
        };
        let (mut blocks, mut liquid, mut biomes) =
            (Box::new([0; SECTION_VOLUME]), Box::new([0; SECTION_VOLUME]), Box::new([0; SECTION_VOLUME]));
        for dx in -1..=1 {
            for dz in -1..=1 {
                let Some(shared) = world.get(cx + dx, cz + dz) else { continue };
                let chunk = shared.read();
                for dy in -1..=1 {
                    let index = section_y + dy - (dim.min_y >> 4);
                    let below = index < 0;
                    let present = !below && chunk.copy_section(index as usize, &mut blocks, &mut liquid);
                    if !present {
                        // Sections the server hasn't sent yet hide faces like unloaded columns do;
                        // their arrival remeshes the neighbours.
                        let unknown = below || (index < chunk.section_count() as i32 && !chunk.section_known(index as usize));
                        blocks.fill(if unknown { OCCLUDER } else { dim.air });
                        liquid.fill(dim.air);
                    }
                    if below || !chunk.copy_biomes(index as usize, &mut biomes) {
                        biomes.fill(NO_BIOME);
                    }
                    v.copy_part([&blocks, &liquid, &biomes], [dx, dy, dz]);
                }
            }
        }
        Some(v)
    }

    /// Copies the part of a neighbouring section (offset -1..=1 per axis) that falls in the border.
    fn copy_part(&mut self, [blocks, liquid, biomes]: [&[u32; SECTION_VOLUME]; 3], offset: [i32; 3]) {
        let range = |o: i32, border: i32| match o {
            -1 => 16 - border..16,
            0 => 0..16,
            _ => 0..border,
        };
        let [ox, oy, oz] = offset;
        for x in range(ox, BLEND) {
            for z in range(oz, BLEND) {
                for y in range(oy, 1) {
                    let src = ((x << 8) | (z << 4) | y) as usize;
                    let at = [x + ox * 16, y + oy * 16, z + oz * 16];
                    self.biomes[biome_cell(at[0], at[1], at[2])] = biomes[src];
                    if range(ox, 1).contains(&x) && range(oz, 1).contains(&z) {
                        let dst = cell(at[0], at[1], at[2]);
                        self.blocks[dst] = blocks[src];
                        self.liquid[dst] = liquid[src];
                    }
                }
            }
        }
    }
}
