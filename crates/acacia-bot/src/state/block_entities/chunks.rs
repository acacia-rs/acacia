//! Block entities carried by `LevelChunk` and `SubChunk` payloads.

use acacia_client::proto::nbt::{self, Nbt, Network, Value};
use acacia_client::proto::packets::{LevelChunk, Subchunk};
use acacia_client::proto::types::SubChunkEntryItemResult;
use acacia_world::{Dimension, level_chunk_block_entities, sub_chunk_block_entities};

use super::BlockEntities;

impl BlockEntities {
    pub(super) fn level_chunk(&mut self, c: LevelChunk) {
        let request_mode = c.highest_subchunk_count.is_some();
        let sections = if request_mode || c.cache_enabled { 0 } else { c.sub_chunk_count };
        let biomes = (!c.cache_enabled).then(|| Dimension::from_id(c.dimension, 0));
        let found = match level_chunk_block_entities(&c.payload, sections, biomes) {
            Ok(offset) => self.parse(&c.payload[offset..]),
            Err(e) => {
                tracing::debug!(x = c.x, z = c.z, error = %e, "level chunk block entities not found");
                return;
            }
        };
        // A full chunk replaces the column; in request mode the sub-chunks do, section by section.
        if !request_mode {
            self.columns.remove(&(c.x, c.z));
        }
        found.into_iter().for_each(|(pos, nbt)| self.insert(pos, nbt));
    }

    pub(super) fn sub_chunk(&mut self, sc: Subchunk) {
        for e in sc.entries {
            let at = |origin: i32, d: i8| origin.wrapping_add(i32::from(d));
            let (x, y, z) = (at(sc.origin.x, e.dx), at(sc.origin.y, e.dy), at(sc.origin.z, e.dz));
            if !matches!(e.result, SubChunkEntryItemResult::Success | SubChunkEntryItemResult::SuccessAllAir) {
                continue;
            }
            if let Some(c) = self.columns.get_mut(&(x, z)) {
                c.retain(|pos, _| pos[1] >> 4 != y);
            }
            let payload = e.payload.unwrap_or_default();
            // With the blob cache the section is the blob, so the payload is only block entities.
            let offset = match (sc.cache_enabled, e.blob_id) {
                (true, Some(_)) => Ok(0),
                _ if payload.is_empty() => continue,
                _ => sub_chunk_block_entities(&payload),
            };
            match offset {
                Ok(offset) => self.parse(&payload[offset..]).into_iter().for_each(|(pos, nbt)| self.insert(pos, nbt)),
                Err(e) => tracing::debug!(x, y, z, error = %e, "sub-chunk block entities not found"),
            }
        }
    }

    /// Concatenated network NBT compounds up to the end; malformed data ends the list.
    fn parse(&self, mut tail: &[u8]) -> Vec<([i32; 3], Nbt)> {
        let mut found = Vec::new();
        while !tail.is_empty() {
            match nbt::read::<Network>(&mut tail) {
                Ok(nbt) => {
                    if let Some(pos) = position(&nbt).filter(|_| self.keeps(&nbt)) {
                        found.push((pos, nbt));
                    }
                }
                Err(e) => {
                    tracing::debug!(error = %e, "malformed block entity in chunk data");
                    break;
                }
            }
        }
        found
    }
}

fn position(nbt: &Nbt) -> Option<[i32; 3]> {
    let int = |key| match nbt.value.get(key) {
        Some(Value::Int(v)) => Some(*v),
        _ => None,
    };
    Some([int("x")?, int("y")?, int("z")?])
}
