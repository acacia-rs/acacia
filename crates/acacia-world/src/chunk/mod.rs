//! Chunk columns decoded from `LevelChunk` / `SubChunk` payloads. Only storage layer 0 (blocks) and
//! layer 1 (liquid, i.e. waterlogging) are kept; biomes, border blocks and block entities are skipped
//! ([`level_chunk_block_entities`] finds the latter).

mod biomes;
mod reader;
mod storage;
mod tail;

pub use storage::VOLUME as SECTION_VOLUME;
pub use tail::{level_chunk_block_entities, sub_chunk_block_entities};

use reader::Reader;
use storage::{Storage, index};

use crate::Error;

/// Vertical extent of a dimension plus the air runtime id used for everything not stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimension {
    pub min_y: i32,
    /// Height in blocks, a multiple of 16.
    pub height: u32,
    pub air: u32,
}

impl Dimension {
    pub const fn overworld(air: u32) -> Self {
        Dimension { min_y: -64, height: 384, air }
    }
    pub const fn nether(air: u32) -> Self {
        Dimension { min_y: 0, height: 128, air }
    }
    pub const fn end(air: u32) -> Self {
        Dimension { min_y: 0, height: 256, air }
    }
    /// From the protocol dimension id (0 overworld, 1 nether, 2 end).
    pub const fn from_id(id: i32, air: u32) -> Self {
        match id {
            1 => Self::nether(air),
            2 => Self::end(air),
            _ => Self::overworld(air),
        }
    }
    fn sections(&self) -> usize {
        (self.height / 16) as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Section {
    blocks: Storage,
    liquid: Storage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub x: i32,
    pub z: i32,
    dim: Dimension,
    sections: Box<[Option<Box<Section>>]>,
    /// Biome ids per section, bottom first; shorter (or empty) until the server sent them.
    biomes: Vec<Storage>,
}

impl Chunk {
    pub fn empty(x: i32, z: i32, dim: Dimension) -> Self {
        Chunk { x, z, dim, sections: vec![None; dim.sections()].into(), biomes: Vec::new() }
    }

    /// Decodes a full `LevelChunk` payload (client cache disabled). `sub_chunk_count` sections come
    /// first; whatever follows (biomes, border, block entities) is ignored.
    pub fn decode(x: i32, z: i32, dim: Dimension, sub_chunk_count: u32, payload: &[u8]) -> Result<Self, Error> {
        Self::decode_mapped(x, z, dim, sub_chunk_count, payload, &|id| id)
    }

    /// [`Chunk::decode`] with palette ids translated by `map` (hashed network ids → runtime ids).
    pub fn decode_mapped(
        x: i32,
        z: i32,
        dim: Dimension,
        sub_chunk_count: u32,
        payload: &[u8],
        map: &dyn Fn(u32) -> u32,
    ) -> Result<Self, Error> {
        let mut chunk = Chunk::empty(x, z, dim);
        let mut r = Reader::new(payload);
        for i in 0..sub_chunk_count {
            let (y_index, section) = decode_section(&mut r, dim.air, map)?;
            let slot = y_index.map_or(i as i32, |y| y as i32 - (dim.min_y >> 4));
            chunk.put_section(slot, section);
        }
        chunk.biomes = biomes::decode(&mut r, dim.sections());
        Ok(chunk)
    }

    /// Replaces the biomes from a payload that is only biome storages (request-mode `LevelChunk`
    /// payload, or the biome blob in cache mode).
    pub fn set_biomes(&mut self, payload: &[u8]) {
        self.biomes = biomes::decode(&mut Reader::new(payload), self.dim.sections());
    }

    /// Unpacks the biome ids of section `index` (0 = lowest) in XZY order. False when unknown.
    pub fn copy_biomes(&self, index: usize, out: &mut [u32; SECTION_VOLUME]) -> bool {
        let Some(storage) = self.biomes.get(index) else { return false };
        storage.copy_into(out);
        true
    }

    /// Biome id at a world position (x/z modulo 16), if known.
    pub fn biome(&self, x: i32, y: i32, z: i32) -> Option<u32> {
        Some(self.biomes.get(self.slot(y)?)?.get(index(x, y, z)))
    }

    /// Decodes one `SubChunk` entry payload into section `section_y` (world y >> 4).
    pub fn set_sub_chunk(&mut self, section_y: i32, payload: &[u8]) -> Result<(), Error> {
        self.set_sub_chunk_mapped(section_y, payload, &|id| id)
    }

    pub fn set_sub_chunk_mapped(&mut self, section_y: i32, payload: &[u8], map: &dyn Fn(u32) -> u32) -> Result<(), Error> {
        let (_, section) = decode_section(&mut Reader::new(payload), self.dim.air, map)?;
        self.put_section(section_y - (self.dim.min_y >> 4), section);
        Ok(())
    }

    pub fn dimension(&self) -> Dimension {
        self.dim
    }

    /// Layer-0 runtime id at a world position (x/z are taken modulo 16).
    pub fn block(&self, x: i32, y: i32, z: i32) -> u32 {
        self.section(y).map_or(self.dim.air, |s| s.blocks.get(index(x, y, z)))
    }

    /// Layer-1 (liquid) runtime id at a world position; air when not waterlogged.
    pub fn liquid(&self, x: i32, y: i32, z: i32) -> u32 {
        self.section(y).map_or(self.dim.air, |s| s.liquid.get(index(x, y, z)))
    }

    /// Applies `UpdateBlock` / `UpdateSubChunkBlocks`. Layers other than 0/1 and out-of-range y are ignored.
    pub fn set(&mut self, x: i32, y: i32, z: i32, layer: u32, id: u32) {
        let Some(slot) = self.slot(y) else { return };
        let air = self.dim.air;
        let s = self.sections[slot]
            .get_or_insert_with(|| Box::new(Section { blocks: Storage::Single(air), liquid: Storage::Single(air) }));
        match layer {
            0 => s.blocks.set(index(x, y, z), id),
            1 => s.liquid.set(index(x, y, z), id),
            _ => {}
        }
    }

    /// Number of 16-block sections from `min_y` up.
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Unpacks section `index` (0 = lowest) into XZY-ordered runtime ids, `(x << 8) | (z << 4) | y`.
    /// Returns false, leaving the buffers untouched, when the section was never sent (all air).
    pub fn copy_section(&self, index: usize, blocks: &mut [u32; SECTION_VOLUME], liquid: &mut [u32; SECTION_VOLUME]) -> bool {
        let Some(s) = self.sections.get(index).and_then(Option::as_deref) else { return false };
        s.blocks.copy_into(blocks);
        s.liquid.copy_into(liquid);
        true
    }

    /// Whether section `index` holds only one block id in both layers (e.g. all stone or all air).
    pub fn section_uniform(&self, index: usize) -> Option<u32> {
        match self.sections.get(index)? {
            None => Some(self.dim.air),
            Some(s) => s.blocks.single().filter(|_| s.liquid.single() == Some(self.dim.air)),
        }
    }

    /// Every runtime id referenced by a palette in this chunk (for validation).
    pub fn palette_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.sections
            .iter()
            .flatten()
            .flat_map(|s| s.blocks.palette().iter().chain(s.liquid.palette()).copied())
    }

    fn slot(&self, y: i32) -> Option<usize> {
        let rel = y - self.dim.min_y;
        (0..self.dim.height as i32).contains(&rel).then_some((rel >> 4) as usize)
    }

    fn section(&self, y: i32) -> Option<&Section> {
        self.sections[self.slot(y)?].as_deref()
    }

    fn put_section(&mut self, slot: i32, section: Section) {
        if let Some(s) = usize::try_from(slot).ok().and_then(|i| self.sections.get_mut(i)) {
            *s = Some(Box::new(section));
        }
    }
}

/// Section versions: 1 (single storage), 8 (storage count), 9 (count + y index).
fn section_header(r: &mut Reader) -> Result<(u8, Option<i8>), Error> {
    match r.u8()? {
        1 => Ok((1, None)),
        8 => Ok((r.u8()?, None)),
        9 => Ok((r.u8()?, Some(r.u8()? as i8))),
        v => Err(Error::SectionVersion(v)),
    }
}

fn decode_section(r: &mut Reader, air: u32, map: &dyn Fn(u32) -> u32) -> Result<(Option<i8>, Section), Error> {
    let (count, y_index) = section_header(r)?;
    let mut section = Section { blocks: Storage::Single(air), liquid: Storage::Single(air) };
    for layer in 0..count {
        let mut storage = Storage::decode(r)?;
        storage.map_ids(map);
        match layer {
            0 => section.blocks = storage,
            1 => section.liquid = storage,
            _ => {}
        }
    }
    Ok((y_index, section))
}
