//! Light data of `LevelChunkWithLight` and `LightUpdate`: four section masks, then the sky and block light arrays.

use crate::{Error, VarInt, Wire};

/// A `java.util.BitSet` on the wire: `toByteArray`, little-endian bytes without trailing zero bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BitSet(pub Vec<u8>);

impl BitSet {
    pub fn from_bits(bits: impl IntoIterator<Item = usize>) -> Self {
        let mut set = Self::default();
        bits.into_iter().for_each(|b| set.set(b));
        set
    }

    pub fn set(&mut self, bit: usize) {
        if self.0.len() <= bit / 8 {
            self.0.resize(bit / 8 + 1, 0);
        }
        self.0[bit / 8] |= 1 << (bit % 8);
    }

    pub fn contains(&self, bit: usize) -> bool {
        self.0.get(bit / 8).is_some_and(|b| b >> (bit % 8) & 1 == 1)
    }

    pub fn bits(&self) -> Vec<usize> {
        (0..self.0.len() * 8).filter(|&b| self.contains(b)).collect()
    }
}

impl Wire for BitSet {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        <Vec<u8>>::read(input).map(Self)
    }

    fn write(&self, out: &mut Vec<u8>) {
        let len = self.0.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        VarInt(len as i32).write(out);
        out.extend_from_slice(&self.0[..len]);
    }
}

/// Bytes of one section's light: a nibble a block.
pub const LIGHT_BYTES: usize = 2048;

/// Light for `section count + 2` slots: bit 0 is the section below the world, the last one above it. A set bit has
/// an array (in bit order); an empty bit is all zero without one.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LightData {
    pub sky_mask: BitSet,
    pub block_mask: BitSet,
    pub empty_sky_mask: BitSet,
    pub empty_block_mask: BitSet,
    pub sky: Vec<Vec<u8>>,
    pub block: Vec<Vec<u8>>,
}

impl LightData {
    /// Adds a slot's sky light; slots must be added in ascending order, since arrays follow bit order.
    pub fn add_sky(&mut self, slot: usize, data: &[u8]) {
        add_layer(&mut self.sky_mask, &mut self.empty_sky_mask, &mut self.sky, slot, data);
    }

    pub fn add_block(&mut self, slot: usize, data: &[u8]) {
        add_layer(&mut self.block_mask, &mut self.empty_block_mask, &mut self.block, slot, data);
    }
}

/// An all-zero array goes in the empty mask instead of the list.
fn add_layer(set: &mut BitSet, empty: &mut BitSet, arrays: &mut Vec<Vec<u8>>, slot: usize, data: &[u8]) {
    if data.iter().all(|&b| b == 0) {
        empty.set(slot);
    } else {
        set.set(slot);
        arrays.push(data.to_vec());
    }
}

fn read_arrays(input: &mut &[u8]) -> Result<Vec<Vec<u8>>, Error> {
    let arrays = <Vec<Vec<u8>>>::read(input)?;
    if arrays.iter().any(|a| a.len() != LIGHT_BYTES) {
        return Err(Error::Invalid("light array length"));
    }
    Ok(arrays)
}

impl Wire for LightData {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self {
            sky_mask: BitSet::read(input)?,
            block_mask: BitSet::read(input)?,
            empty_sky_mask: BitSet::read(input)?,
            empty_block_mask: BitSet::read(input)?,
            sky: read_arrays(input)?,
            block: read_arrays(input)?,
        })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.sky_mask.write(out);
        self.block_mask.write(out);
        self.empty_sky_mask.write(out);
        self.empty_block_mask.write(out);
        self.sky.write(out);
        self.block.write(out);
    }
}
