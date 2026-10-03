//! Per-section biome ids: one paletted storage per dimension section, bottom first, in the block
//! storage format (ids are biome ids, never remapped). They follow the sections of a full
//! `LevelChunk`, make up the whole payload in sub-chunk request mode, and form the biome blob in
//! cache mode.

use std::borrow::Cow;

use super::reader::Reader;
use super::storage::Storage;

/// Header meaning "same as the section below" (bits 127, runtime palette).
pub(super) const COPY_BELOW: u8 = 0xff;

/// Writes one storage per section. A section equal to the one below it is the marker; sections past
/// the known ones repeat the top one (biome 0 when none are known).
pub(super) fn encode(biomes: &[Storage], sections: usize, out: &mut Vec<u8>) {
    let mut below: Option<Cow<Storage>> = None;
    for i in 0..sections {
        let storage = match biomes.get(i) {
            Some(s) => s.compacted(),
            None => below.clone().unwrap_or(Cow::Owned(Storage::Single(0))),
        };
        if below.as_ref() == Some(&storage) {
            out.push(COPY_BELOW);
        } else {
            storage.encode(out, &|id| id);
        }
        below = Some(storage);
    }
}

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
