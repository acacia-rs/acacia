//! Where block entities start in chunk payloads, found by skipping (not decoding) what precedes them.
//! They are concatenated network NBT compounds running to the end of the payload.

use super::reader::Reader;
use super::storage::Storage;
use super::{Dimension, section_header};
use crate::Error;

/// Biome storage header meaning "same as the section below" (bits 127, runtime palette).
const BIOME_COPY: u8 = 0xff;

/// Offset of the block entities in a `LevelChunk` payload: after `sections` sub-chunks, one biome
/// storage per section of `biomes` (`None` in cache mode, where biomes are a blob) and the border
/// blocks. Sub-chunk request mode has no sections; a payload that ends after the biomes has none.
pub fn level_chunk_block_entities(payload: &[u8], sections: u32, biomes: Option<Dimension>) -> Result<usize, Error> {
    let mut r = Reader::new(payload);
    for _ in 0..sections {
        skip_section(&mut r)?;
    }
    for _ in 0..biomes.map_or(0, |d| d.sections()) {
        let header = r.u8()?;
        if header != BIOME_COPY {
            Storage::skip(&mut r, header)?;
        }
    }
    if r.remaining() > 0 {
        let border = r.u8()?;
        r.bytes(border as usize)?;
    }
    Ok(payload.len() - r.remaining())
}

/// Offset of the block entities in a `SubChunk` entry payload sent without the blob cache (the
/// section comes first; with the cache the whole payload is block entities).
pub fn sub_chunk_block_entities(payload: &[u8]) -> Result<usize, Error> {
    let mut r = Reader::new(payload);
    skip_section(&mut r)?;
    Ok(payload.len() - r.remaining())
}

fn skip_section(r: &mut Reader) -> Result<(), Error> {
    let (count, _) = section_header(r)?;
    for _ in 0..count {
        let header = r.u8()?;
        Storage::skip(r, header)?;
    }
    Ok(())
}
