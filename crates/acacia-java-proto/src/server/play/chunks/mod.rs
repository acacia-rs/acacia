//! Chunk, block, world and dimension packets a server sends in the play state, and the chunk data writer.
//! Layouts were checked with `javap -p -c` on the 26.3 codec classes; every body is a [`Wire`](crate::Wire), so each
//! round-trips. Ids are in [`crate::ids::play::clientbound`].

pub mod batch;
pub mod blocks;
pub mod chunk_packet;
pub mod effects;
pub mod heightmap;
pub mod light;
pub mod particles;
pub mod sections;
pub mod world;
pub mod world_border;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_world;

use crate::{Error, Wire};

/// `ByteBufCodecs.VAR_LONG`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VarLong(pub i64);

impl Wire for VarLong {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        crate::read_varlong(input).map(VarLong)
    }

    fn write(&self, out: &mut Vec<u8>) {
        write_varlong(self.0, out);
    }
}

pub(crate) fn write_varlong(value: i64, out: &mut Vec<u8>) {
    let mut v = value as u64;
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

/// `ByteBufCodecs.optional`: a presence flag, then the value.
pub(crate) fn read_option<T: Wire>(input: &mut &[u8]) -> Result<Option<T>, Error> {
    if bool::read(input)? { T::read(input).map(Some) } else { Ok(None) }
}

pub(crate) fn write_option<T: Wire>(value: &Option<T>, out: &mut Vec<u8>) {
    value.is_some().write(out);
    if let Some(v) = value {
        v.write(out);
    }
}

/// Smallest `n` with `2^n >= value`, as `Mth.ceillog2`.
pub fn ceil_log2(value: usize) -> u8 {
    (usize::BITS - value.saturating_sub(1).leading_zeros()) as u8
}
