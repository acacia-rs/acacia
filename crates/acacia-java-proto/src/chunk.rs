//! `LevelChunkWithLight` sections: paletted block and biome containers (`PalettedContainer`, `Strategy`).

use crate::{Error, VarInt, Wire, read_varint};

const BLOCKS: usize = 4096;
const BIOMES: usize = 64;

/// A container's values: one for the whole section, or one per entry in `y z x` order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Values {
    Single(u32),
    Each(Box<[u32]>),
}

impl Values {
    pub fn get(&self, index: usize) -> u32 {
        match self {
            Values::Single(v) => *v,
            Values::Each(all) => all[index],
        }
    }

    pub fn set(&mut self, index: usize, value: u32, len: usize) {
        if let Values::Single(v) = *self {
            if v == value {
                return;
            }
            *self = Values::Each(vec![v; len].into());
        }
        if let Values::Each(all) = self {
            all[index] = value;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// The server's own count of non-air blocks; [`Section::count_where`] can check a decode against it.
    pub non_empty: i16,
    pub fluids: i16,
    pub blocks: Values,
    pub biomes: Values,
}

impl Section {
    /// Index of a section-relative cell.
    pub fn index(x: usize, y: usize, z: usize) -> usize {
        (y << 8) | (z << 4) | x
    }

    pub fn set_block(&mut self, index: usize, state: u32) {
        self.blocks.set(index, state, BLOCKS);
    }

    pub fn count_where(&self, pred: impl Fn(u32) -> bool) -> usize {
        match &self.blocks {
            Values::Single(v) => if pred(*v) { BLOCKS } else { 0 },
            Values::Each(all) => all.iter().filter(|v| pred(**v)).count(),
        }
    }
}

/// Bits a value occupies in storage for the `bits` the server sent; the linear palette for blocks is always 4 wide.
fn storage_bits(bits: u32, entries: usize) -> u32 {
    if entries == BLOCKS && (1..=4).contains(&bits) { 4 } else { bits }
}

fn read_container(input: &mut &[u8], entries: usize, palette_max: u32) -> Result<Values, Error> {
    let bits = u32::from(u8::read(input)?);
    if bits == 0 {
        return Ok(Values::Single(read_varint(input)? as u32));
    }
    let palette = if bits <= palette_max {
        Some(<Vec<VarInt>>::read(input)?)
    } else {
        None
    };
    let width = storage_bits(bits, entries);
    if width > 32 {
        return Err(Error::BadBits(bits));
    }
    let per_long = (64 / width) as usize;
    let mask = (1u64 << width) - 1;
    let mut out = Vec::with_capacity(entries);
    for _ in 0..entries.div_ceil(per_long) {
        let mut long = i64::read(input)? as u64;
        for _ in 0..per_long.min(entries - out.len()) {
            let raw = (long & mask) as u32;
            long >>= width;
            out.push(match &palette {
                Some(p) => p.get(raw as usize).ok_or(Error::BadPaletteIndex(raw))?.0 as u32,
                None => raw,
            });
        }
    }
    Ok(Values::Each(out.into()))
}

/// Every section in a chunk's `buffer`, lowest first.
pub fn read_sections(mut input: &[u8]) -> Result<Vec<Section>, Error> {
    let mut sections = Vec::new();
    while !input.is_empty() {
        sections.push(Section {
            non_empty: i16::read(&mut input)?,
            fluids: i16::read(&mut input)?,
            blocks: read_container(&mut input, BLOCKS, 8)?,
            biomes: read_container(&mut input, BIOMES, 3)?,
        });
    }
    Ok(sections)
}

/// The start of `LevelChunkWithLight`: position and section buffer. Block entities and light follow, unread.
pub fn read_chunk_header(input: &mut &[u8]) -> Result<(i32, i32, Vec<u8>), Error> {
    let (x, z) = (i32::read(input)?, i32::read(input)?);
    for _ in 0..read_varint(input)? {
        read_varint(input)?;
        <Vec<i64>>::read(input)?;
    }
    Ok((x, z, <Vec<u8>>::read(input)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write_varint;

    fn single(out: &mut Vec<u8>, value: i32) {
        out.push(0);
        write_varint(value, out);
    }

    #[test]
    fn single_value_section_decodes() {
        let mut buf = vec![0x10, 0x00, 0, 0];
        single(&mut buf, 7);
        single(&mut buf, 3);
        let s = &read_sections(&buf).unwrap()[0];
        assert_eq!((s.non_empty, s.blocks.get(4095), s.biomes.get(0)), (4096, 7, 3));
    }

    #[test]
    fn palette_entries_do_not_span_longs() {
        // Five bits: twelve values a long, four bits of padding; entry 12 starts the second long.
        let mut buf = vec![0, 2, 0, 0, 5];
        write_varint(2, &mut buf);
        write_varint(9, &mut buf);
        write_varint(300, &mut buf);
        let longs = 4096usize.div_ceil(12);
        for l in 0..longs {
            let v: u64 = if l == 1 { 1 } else { 0 };
            buf.extend_from_slice(&v.to_be_bytes());
        }
        single(&mut buf, 0);
        let s = &read_sections(&buf).unwrap()[0];
        assert_eq!((s.blocks.get(11), s.blocks.get(12), s.blocks.get(13)), (9, 300, 9));
    }

    #[test]
    fn low_block_bits_are_stored_four_wide() {
        // A one-bit header still means a four-bit linear palette; sixteen values to a long.
        let mut buf = vec![0, 1, 0, 0, 1];
        write_varint(2, &mut buf);
        write_varint(5, &mut buf);
        write_varint(6, &mut buf);
        for l in 0..4096 / 16 {
            let v: u64 = if l == 0 { 1 << 4 } else { 0 };
            buf.extend_from_slice(&v.to_be_bytes());
        }
        single(&mut buf, 0);
        let s = &read_sections(&buf).unwrap()[0];
        assert_eq!((s.blocks.get(0), s.blocks.get(1)), (5, 6));
    }

    #[test]
    fn setting_a_single_value_section_expands_it() {
        let mut v = Values::Single(1);
        v.set(5, 1, BLOCKS);
        assert_eq!(v, Values::Single(1));
        v.set(5, 2, BLOCKS);
        assert_eq!((v.get(5), v.get(6)), (2, 1));
    }

    #[test]
    fn bad_palette_index_is_an_error() {
        let mut buf = vec![0, 0, 0, 0, 4];
        write_varint(1, &mut buf);
        write_varint(9, &mut buf);
        for _ in 0..4096 / 16 {
            buf.extend_from_slice(&u64::MAX.to_be_bytes());
        }
        assert_eq!(read_sections(&buf), Err(Error::BadPaletteIndex(15)));
    }
}
