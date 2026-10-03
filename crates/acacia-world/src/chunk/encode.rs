//! Chunk payloads for a server to send: the inverse of the decoders in this module, for protocol
//! 2193 (26.50). Sections are written in version 9; every palette is compacted first.
//!
//! `map` translates runtime ids to wire ids, the inverse of the `map` of [`Chunk::decode_mapped`]:
//! `&|id| id` for runtime ids, `&|id| registry.network_hash(id)` when `block_network_ids_are_hashes`.
//! `block_entities` is the concatenated network NBT of the chunk's (or sub-chunk's) block entities.
//!
//! Blob-cache layout (docs/research/blob-cache.md §2): each section and the biomes become a blob
//! named by its xxh64; the payload keeps what is not cached (border blocks, block entities).

use std::borrow::Cow;

use acacia_proto::packets::LevelChunk;
use acacia_proto::types::{Blob, SubChunkEntryItem, SubChunkEntryItemResult};
use acacia_proto::xxh64;
use bytes::Bytes;

use super::heightmap::{ColumnHeights, Heightmap};
use super::storage::Storage;
use super::{Chunk, Section, biomes};

const SECTION_VERSION: u8 = 9;
/// Border-block count; they exist in Education Edition only.
const NO_BORDER_BLOCKS: u8 = 0;

/// The `LevelChunk` fields that depend on the chunk's content.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelChunkData {
    pub sub_chunk_count: u32,
    /// `Some` puts the client in sub-chunk request mode: it asks for this many sections from the
    /// bottom up with `SubchunkRequest`.
    pub highest_subchunk_count: Option<i32>,
    /// Cache mode only: the blobs to advertise, and to deliver in `ClientCacheMissResponse` to
    /// clients that report them missing. Sections bottom first, then the biomes.
    pub blobs: Vec<Blob>,
    pub payload: Bytes,
}

impl LevelChunkData {
    pub fn packet(&self, x: i32, z: i32, dimension: i32) -> LevelChunk {
        LevelChunk {
            x,
            z,
            dimension,
            sub_chunk_count: self.sub_chunk_count,
            highest_subchunk_count: self.highest_subchunk_count,
            cache_enabled: !self.blobs.is_empty(),
            blobs: self.blobs.iter().map(|b| b.hash).collect(),
            payload: self.payload.clone(),
        }
    }
}

/// One section's answer to a `SubchunkRequest`.
#[derive(Debug, Clone, PartialEq)]
pub struct SubChunkData {
    /// `None` when the section is all air.
    pub payload: Option<Bytes>,
    /// Cache mode only: the section, which `payload` then leaves out.
    pub blob: Option<Blob>,
    pub heightmap: Heightmap,
}

impl SubChunkData {
    /// The entry at an offset from the request's origin. The render heightmap repeats the heightmap.
    pub fn entry(&self, dx: i8, dy: i8, dz: i8) -> SubChunkEntryItem {
        let result = match self.payload {
            Some(_) => SubChunkEntryItemResult::Success,
            None => SubChunkEntryItemResult::SuccessAllAir,
        };
        SubChunkEntryItem {
            dx,
            dy,
            dz,
            result,
            payload: self.payload.clone(),
            heightmap_type: self.heightmap.data_type(),
            heightmap: self.heightmap.rows(),
            render_heightmap_type: self.heightmap.data_type(),
            render_heightmap: self.heightmap.rows(),
            blob_id: self.blob.as_ref().map(|b| b.hash),
        }
    }
}

struct Layers<'a> {
    blocks: Cow<'a, Storage>,
    liquid: Cow<'a, Storage>,
}

impl Section {
    /// Compacted layers, or `None` when both are all air.
    fn layers(&self, air: u32) -> Option<Layers<'_>> {
        let layers = Layers { blocks: self.blocks.compacted(), liquid: self.liquid.compacted() };
        let is_air = layers.blocks.single() == Some(air) && layers.liquid.single() == Some(air);
        (!is_air).then_some(layers)
    }
}

/// `None` writes an all-air section: no storages, as Geyser sends them. The liquid layer is written
/// only when it holds something.
fn write_section(out: &mut Vec<u8>, y_index: i32, layers: Option<&Layers>, air: u32, map: &dyn Fn(u32) -> u32) {
    let blocks = layers.map(|l| &*l.blocks);
    let liquid = layers.map(|l| &*l.liquid).filter(|l| l.single() != Some(air));
    let storages = [blocks, liquid].into_iter().flatten();
    out.extend([SECTION_VERSION, storages.clone().count() as u8, y_index as i8 as u8]);
    storages.for_each(|s| s.encode(out, map));
}

fn blob(data: Vec<u8>) -> Blob {
    Blob { hash: xxh64(&data), payload: data.into() }
}

fn tail(block_entities: &[u8]) -> Vec<u8> {
    [&[NO_BORDER_BLOCKS][..], block_entities].concat()
}

impl Chunk {
    /// Sections from the bottom up to the highest one that is not all air: `sub_chunk_count` of a
    /// full `LevelChunk`, `highest_subchunk_count` in request mode.
    pub fn sub_chunk_count(&self) -> u32 {
        let air = self.dim.air;
        let filled = |s: &Option<Box<Section>>| s.as_deref().is_some_and(|s| s.layers(air).is_some());
        self.sections.iter().rposition(filled).map_or(0, |top| top as u32 + 1)
    }

    fn section_bytes(&self, slot: usize, layers: Option<&Layers>, map: &dyn Fn(u32) -> u32) -> Vec<u8> {
        let mut out = Vec::new();
        write_section(&mut out, (self.dim.min_y >> 4) + slot as i32, layers, self.dim.air, map);
        out
    }

    /// The first [`Chunk::sub_chunk_count`] sections, each encoded on its own.
    fn sections_bytes(&self, map: &dyn Fn(u32) -> u32) -> Vec<Vec<u8>> {
        let count = self.sub_chunk_count() as usize;
        let encode = |(slot, s): (usize, &Option<Box<Section>>)| {
            let layers = s.as_deref().and_then(|s| s.layers(self.dim.air));
            self.section_bytes(slot, layers.as_ref(), map)
        };
        self.sections[..count].iter().enumerate().map(encode).collect()
    }

    /// One biome storage per section of the dimension: the biome blob, and the start of a
    /// request-mode payload.
    pub fn biome_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        biomes::encode(&self.biomes, self.dim.sections(), &mut out);
        out
    }

    /// A full chunk: payload = sections, biomes, border blocks, block entities.
    pub fn level_chunk(&self, block_entities: &[u8], map: &dyn Fn(u32) -> u32) -> LevelChunkData {
        let sections = self.sections_bytes(map);
        let sub_chunk_count = sections.len() as u32;
        let payload = [sections.concat(), self.biome_bytes(), tail(block_entities)].concat();
        LevelChunkData { sub_chunk_count, highest_subchunk_count: None, blobs: Vec::new(), payload: payload.into() }
    }

    /// [`Chunk::level_chunk`] for a client with the blob cache: one blob per section plus the biomes.
    pub fn level_chunk_cached(&self, block_entities: &[u8], map: &dyn Fn(u32) -> u32) -> LevelChunkData {
        let mut blobs: Vec<Blob> = self.sections_bytes(map).into_iter().map(blob).collect();
        let sub_chunk_count = blobs.len() as u32;
        blobs.push(blob(self.biome_bytes()));
        LevelChunkData { sub_chunk_count, highest_subchunk_count: None, blobs, payload: tail(block_entities).into() }
    }

    /// Request mode: only the biomes travel; sections follow from [`Chunk::sub_chunk`].
    pub fn level_chunk_request(&self) -> LevelChunkData {
        let payload = [self.biome_bytes(), tail(&[])].concat();
        LevelChunkData { sub_chunk_count: 0, highest_subchunk_count: Some(self.sub_chunk_count() as i32), blobs: Vec::new(), payload: payload.into() }
    }

    /// [`Chunk::level_chunk_request`] for a client with the blob cache: the biomes are the only blob.
    pub fn level_chunk_request_cached(&self) -> LevelChunkData {
        let blobs = vec![blob(self.biome_bytes())];
        LevelChunkData { sub_chunk_count: 0, highest_subchunk_count: Some(self.sub_chunk_count() as i32), blobs, payload: tail(&[]).into() }
    }

    /// Section `section_y` (world y >> 4) for a `Subchunk` entry: payload = section, block
    /// entities. `None` when the dimension has no such section (`YIndexOutOfBounds`).
    pub fn sub_chunk(&self, section_y: i32, heights: &ColumnHeights, block_entities: &[u8], map: &dyn Fn(u32) -> u32) -> Option<SubChunkData> {
        let section = self.sub_chunk_section(section_y, map)?;
        let payload = section.map(|s| [&s[..], block_entities].concat().into());
        Some(SubChunkData { payload, blob: None, heightmap: heights.sub_chunk(section_y) })
    }

    /// [`Chunk::sub_chunk`] for a client with the blob cache (`Subchunk.cache_enabled`): the section
    /// is the blob and the payload keeps the block entities.
    pub fn sub_chunk_cached(&self, section_y: i32, heights: &ColumnHeights, block_entities: &[u8], map: &dyn Fn(u32) -> u32) -> Option<SubChunkData> {
        let section = self.sub_chunk_section(section_y, map)?;
        let payload = section.is_some().then(|| Bytes::copy_from_slice(block_entities));
        Some(SubChunkData { payload, blob: section.map(blob), heightmap: heights.sub_chunk(section_y) })
    }

    /// Outer `None`: out of range. Inner `None`: all air.
    fn sub_chunk_section(&self, section_y: i32, map: &dyn Fn(u32) -> u32) -> Option<Option<Vec<u8>>> {
        let slot = usize::try_from(section_y - (self.dim.min_y >> 4)).ok()?;
        let layers = self.sections.get(slot)?.as_deref().and_then(|s| s.layers(self.dim.air));
        Some(layers.map(|l| self.section_bytes(slot, Some(&l), map)))
    }
}
