//! Chunk heightmaps: a `Heightmap.Types` id and the raw `SimpleBitStorage` longs.

use super::ceil_log2;
use crate::types::HeightmapKind;
use crate::{Error, Wire};

/// The types with `Usage.CLIENT`; `ClientboundLevelChunkPacketData` filters on `sendToClient`, so these are all
/// a vanilla server sends (wire ids 1, 4, 5).
pub const CLIENT_KINDS: [HeightmapKind; 3] = [HeightmapKind::WorldSurface, HeightmapKind::MotionBlocking, HeightmapKind::MotionBlockingNoLeaves];

pub const COLUMNS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Heightmap {
    pub kind: HeightmapKind,
    pub data: Vec<i64>,
}

impl Heightmap {
    /// `heights` indexed `z * 16 + x`: one above the highest matching block, counted from the world bottom.
    pub fn from_heights(kind: HeightmapKind, heights: &[u16], world_height: u32) -> Self {
        Self { kind, data: pack_heights(heights, bits_for(world_height)) }
    }

    pub fn heights(&self, world_height: u32) -> Result<Vec<u16>, Error> {
        unpack_heights(&self.data, bits_for(world_height))
    }
}

impl Wire for Heightmap {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { kind: HeightmapKind::read(input)?, data: Vec::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.kind.write(out);
        self.data.write(out);
    }
}

/// `Heightmap` stores `ceillog2(height + 1)` bits per column.
pub fn bits_for(world_height: u32) -> u32 {
    u32::from(ceil_log2(world_height as usize + 1))
}

fn longs_for(bits: u32) -> usize {
    COLUMNS.div_ceil((64 / bits) as usize)
}

/// Entries never span two longs.
pub fn pack_heights(heights: &[u16], bits: u32) -> Vec<i64> {
    let per_long = (64 / bits) as usize;
    heights
        .chunks(per_long)
        .map(|chunk| chunk.iter().enumerate().fold(0u64, |long, (i, &h)| long | u64::from(h) << (i as u32 * bits)) as i64)
        .collect()
}

pub fn unpack_heights(data: &[i64], bits: u32) -> Result<Vec<u16>, Error> {
    if bits == 0 || bits > 16 || data.len() != longs_for(bits) {
        return Err(Error::Invalid("heightmap storage"));
    }
    let per_long = (64 / bits) as usize;
    let mask = (1u64 << bits) - 1;
    let mut out = Vec::with_capacity(COLUMNS);
    for &long in data {
        for i in 0..per_long.min(COLUMNS - out.len()) {
            out.push(((long as u64 >> (i as u32 * bits)) & mask) as u16);
        }
    }
    Ok(out)
}
