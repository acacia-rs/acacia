//! One chunk column in Java states, converted from an `acacia_world::Chunk`.

use acacia_world::{BlockFlags, BlockRegistry, Chunk, SECTION_VOLUME};

use crate::mapping::{self, AIR, JavaState, StateMap};
use crate::section::{Blocks, VOLUME, index};

/// Biome cells of a section: 4×4×4, index `(qy << 4) | (qz << 2) | qx`.
pub type SectionBiomes = [u16; 64];

pub struct JavaColumn {
    pub x: i32,
    pub z: i32,
    /// Section y of `sections[0]` (−4 in the overworld).
    pub min_section: i32,
    pub sections: Vec<Blocks>,
    pub biomes: Vec<SectionBiomes>,
}

impl JavaColumn {
    pub fn min_y(&self) -> i32 {
        self.min_section * 16
    }

    pub fn height(&self) -> i32 {
        self.sections.len() as i32 * 16
    }

    /// Local x/z, world y; outside the column's height reads as air.
    #[inline]
    pub fn block(&self, x: usize, y: i32, z: usize) -> JavaState {
        let sy = (y >> 4) - self.min_section;
        match self.sections.get(sy as usize) {
            Some(s) if sy >= 0 => s.get(index(x, (y & 15) as usize, z)),
            _ => AIR,
        }
    }

    /// Sets blocks (local x/z, y above the column's bottom), unpacking each touched section once.
    pub fn apply_edits(
        &mut self,
        edits: &mut [([usize; 3], JavaState)],
        buf: &mut [JavaState; VOLUME],
    ) {
        edits.sort_unstable_by_key(|([_, y, _], _)| *y);
        for group in edits.chunk_by(|a, b| a.0[1] >> 4 == b.0[1] >> 4) {
            let Some(section) = self.sections.get_mut(group[0].0[1] >> 4) else {
                continue;
            };
            section.copy_to(buf);
            for &([x, y, z], s) in group {
                buf[index(x, y & 15, z)] = s;
            }
            *section = Blocks::from_full(buf);
        }
    }

    pub fn heap_bytes(&self) -> usize {
        self.sections.iter().map(Blocks::heap_bytes).sum::<usize>() + self.biomes.len() * 128 + 64
    }
}

/// Reusable buffers for one conversion thread.
pub struct Scratch {
    blocks: Box<[u32; SECTION_VOLUME]>,
    liquid: Box<[u32; SECTION_VOLUME]>,
    java: Box<[JavaState; VOLUME]>,
    biomes: Box<[u32; SECTION_VOLUME]>,
}

impl Default for Scratch {
    fn default() -> Self {
        Self {
            blocks: Box::new([0; SECTION_VOLUME]),
            liquid: Box::new([0; SECTION_VOLUME]),
            java: Box::new([0; VOLUME]),
            biomes: Box::new([0; SECTION_VOLUME]),
        }
    }
}

/// Maps one registry's Bedrock chunks to Java columns.
pub struct Converter {
    pub map: StateMap,
    /// Per runtime id: the Java state when the liquid layer holds water (waterlogged variant, or the
    /// block itself for always-wet plants), or `None` for blocks that are not water-capable.
    wet: Vec<Option<JavaState>>,
    is_water: Vec<bool>,
}

impl Converter {
    pub fn new(registry: &BlockRegistry) -> Self {
        let map = StateMap::new(registry);
        let j = mapping::java();
        let n = registry.len() as u32;
        let is_water: Vec<bool> = (0..n)
            .map(|id| {
                registry
                    .get(id)
                    .is_some_and(|s| s.flags.contains(BlockFlags::WATER))
            })
            .collect();
        let wet = (0..n)
            .map(|id| {
                let s = map.java(id);
                j.with(s, "waterlogged", "true")
                    .or_else(|| (s != AIR && is_water[id as usize]).then_some(s))
            })
            .collect();
        Self { map, wet, is_water }
    }

    #[inline]
    fn state(&self, block: u32, liquid: u32) -> JavaState {
        if self.is_water.get(liquid as usize).copied().unwrap_or(false) {
            let java = self.map.java(block);
            if java == AIR {
                return self.map.java(liquid);
            }
            return self
                .wet
                .get(block as usize)
                .copied()
                .flatten()
                .unwrap_or(java);
        }
        self.map.java(block)
    }

    pub fn convert(&self, chunk: &Chunk, s: &mut Scratch) -> JavaColumn {
        let dim = chunk.dimension();
        let count = chunk.section_count();
        let biome_map = mapping::biomes();
        let mut sections = Vec::with_capacity(count);
        let mut biomes = Vec::with_capacity(count);
        let mut prev_biomes = [biome_map.java(1); 64];
        for i in 0..count {
            sections.push(self.convert_section(chunk, i, s));
            if chunk.copy_biomes(i, &mut s.biomes) {
                for (q, cell) in prev_biomes.iter_mut().enumerate() {
                    let (qx, qz, qy) = (q & 3, (q >> 2) & 3, q >> 4);
                    *cell = biome_map.java(s.biomes[(qx * 4) << 8 | (qz * 4) << 4 | (qy * 4)]);
                }
            }
            biomes.push(prev_biomes);
        }
        JavaColumn {
            x: chunk.x,
            z: chunk.z,
            min_section: dim.min_y >> 4,
            sections,
            biomes,
        }
    }

    fn convert_section(&self, chunk: &Chunk, i: usize, s: &mut Scratch) -> Blocks {
        if let Some(id) = chunk.section_uniform(i) {
            return Blocks::Uniform(self.state(id, u32::MAX));
        }
        if !chunk.copy_section(i, &mut s.blocks, &mut s.liquid) {
            return Blocks::Uniform(AIR);
        }
        // Bedrock is XZY, Java YZX.
        for x in 0..16 {
            for z in 0..16 {
                let col = (x << 8) | (z << 4);
                for y in 0..16 {
                    s.java[index(x, y, z)] = self.state(s.blocks[col | y], s.liquid[col | y]);
                }
            }
        }
        Blocks::from_full(&s.java)
    }
}
