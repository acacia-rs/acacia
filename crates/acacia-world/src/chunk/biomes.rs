//! Per-section biome ids: one paletted storage per dimension section, bottom first, in the block
//! storage format (ids are biome ids, never remapped). They follow the sections of a full
//! `LevelChunk`, make up the whole payload in sub-chunk request mode, and form the biome blob in
//! cache mode.

use super::reader::Reader;
use super::storage::Storage;

/// Header meaning "same as the section below" (bits 127, runtime palette).
pub(super) const COPY_BELOW: u8 = 0xff;

/// Decodes up to `sections` storages. Biomes only tint, so malformed or truncated data keeps what
/// decoded before it instead of failing the chunk.
pub(super) fn decode(r: &mut Reader, sections: usize) -> Vec<Storage> {
    let mut out: Vec<Storage> = Vec::with_capacity(sections);
    while out.len() < sections {
        let Ok(header) = r.u8() else { break };
        let storage = if header == COPY_BELOW {
            out.last().cloned().unwrap_or(Storage::Single(0))
        } else {
            match Storage::decode_after(r, header) {
                Ok(s) => s,
                Err(_) => break,
            }
        };
        out.push(storage);
    }
    out
}
