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
    pub biomes: Box<[u32; CELLS]>,
}

/// Cell index for section-local coordinates in -1..=16.
#[inline]
pub fn cell(x: i32, y: i32, z: i32) -> usize {
    (((x + 1) as usize * SIDE) + (z + 1) as usize) * SIDE + (y + 1) as usize
}

impl Volume {
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
        self.biomes[cell(x, y, z)]
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
            biomes: Box::new([NO_BIOME; CELLS]),
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
        let range = |o: i32| match o {
            -1 => 15..16,
            0 => 0..16,
            _ => 0..1,
        };
        let [ox, oy, oz] = offset;
        for x in range(ox) {
            for z in range(oz) {
                for y in range(oy) {
                    let src = ((x << 8) | (z << 4) | y) as usize;
                    let dst = cell(x + ox * 16, y + oy * 16, z + oz * 16);
                    self.blocks[dst] = blocks[src];
                    self.liquid[dst] = liquid[src];
                    self.biomes[dst] = biomes[src];
                }
            }
        }
    }
}
