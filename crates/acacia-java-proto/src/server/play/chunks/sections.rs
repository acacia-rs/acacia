//! The chunk section buffer writer: the inverse of [`crate::chunk::read_sections`].
//!
//! Per section: non-empty block count, fluid count (both shorts), then the block and biome paletted containers.
//! A container is a bits byte, a palette (single value or linear list; none for the global palette), then the
//! storage longs (no length prefix) with entries never spanning two longs. `Strategy` rules: blocks use 4-bit linear
//! palettes up to 8 bits then the global palette; biomes 1..=3 bits then global. Entry order is `y z x`.

use std::collections::HashMap;

use super::ceil_log2;
use crate::chunk::{Section, Values};
use crate::{VarInt, Wire, write_varint};

const BLOCKS: usize = 4096;
const BIOMES: usize = 64;

/// Global palette widths: `ceillog2` of each registry's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Widths {
    pub block_bits: u8,
    pub biome_bits: u8,
}

impl Widths {
    /// 35723 block states in 26.3's `reports/blocks.json`.
    pub const BLOCK_STATES_26_3: u8 = 16;

    /// Vanilla block states, and a biome registry of `biome_count` entries.
    pub fn new(biome_count: usize) -> Self {
        Self { block_bits: Self::BLOCK_STATES_26_3, biome_bits: ceil_log2(biome_count) }
    }
}

struct Strategy {
    entries: usize,
    min_bits: u8,
    max_linear: u8,
    global_bits: u8,
}

impl Section {
    /// Sets both counts from the contents: `is_air` for the non-empty count, `is_fluid` for states with a fluid.
    pub fn recount(&mut self, is_air: impl Fn(u32) -> bool, is_fluid: impl Fn(u32) -> bool) {
        self.non_empty = (BLOCKS - self.count_where(is_air)) as i16;
        self.fluids = self.count_where(is_fluid) as i16;
    }
}

/// `sections` lowest first, as the chunk packet's `buffer`.
pub fn write_sections(sections: &[Section], widths: Widths, out: &mut Vec<u8>) {
    let blocks = Strategy { entries: BLOCKS, min_bits: 4, max_linear: 8, global_bits: widths.block_bits };
    let biomes = Strategy { entries: BIOMES, min_bits: 1, max_linear: 3, global_bits: widths.biome_bits };
    for section in sections {
        section.non_empty.write(out);
        section.fluids.write(out);
        write_container(&section.blocks, &blocks, out);
        write_container(&section.biomes, &biomes, out);
    }
}

fn write_container(values: &Values, strategy: &Strategy, out: &mut Vec<u8>) {
    let all = match values {
        Values::Single(v) => return write_single(*v, out),
        Values::Each(all) => all,
    };
    debug_assert_eq!(all.len(), strategy.entries);
    let mut palette: Vec<u32> = Vec::new();
    let mut index: HashMap<u32, u64> = HashMap::new();
    for &v in all.iter() {
        index.entry(v).or_insert_with(|| {
            palette.push(v);
            palette.len() as u64 - 1
        });
    }
    if palette.len() == 1 {
        return write_single(palette[0], out);
    }
    let bits = ceil_log2(palette.len()).max(strategy.min_bits);
    if bits <= strategy.max_linear {
        out.push(bits);
        write_varint(palette.len() as i32, out);
        palette.iter().for_each(|&v| VarInt(v as i32).write(out));
        pack(all.iter().map(|v| index[v]), u32::from(bits), out);
    } else {
        out.push(strategy.global_bits);
        pack(all.iter().map(|&v| u64::from(v)), u32::from(strategy.global_bits), out);
    }
}

fn write_single(value: u32, out: &mut Vec<u8>) {
    out.push(0);
    write_varint(value as i32, out);
}

/// `SimpleBitStorage`: `64 / bits` entries a long, lowest first.
fn pack(entries: impl Iterator<Item = u64>, bits: u32, out: &mut Vec<u8>) {
    let per_long = (64 / bits) as usize;
    let (mut long, mut filled) = (0u64, 0usize);
    for entry in entries {
        debug_assert!(entry >> bits == 0, "entry {entry} wider than {bits} bits");
        long |= entry << (filled as u32 * bits);
        filled += 1;
        if filled == per_long {
            out.extend_from_slice(&long.to_be_bytes());
            (long, filled) = (0, 0);
        }
    }
    if filled > 0 {
        out.extend_from_slice(&long.to_be_bytes());
    }
}
