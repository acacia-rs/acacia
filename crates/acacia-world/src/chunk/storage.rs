//! One 16³ paletted block storage kept packed as on the wire. Index order is XZY: `(x << 8) | (z << 4) | y`.

use super::reader::Reader;
use crate::Error;

pub const VOLUME: usize = 4096;
const VALID_BITS: [u8; 8] = [1, 2, 3, 4, 5, 6, 8, 16];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Storage {
    Single(u32),
    Packed {
        bits: u8,
        palette: Vec<u32>,
        words: Box<[u32]>,
    },
}

// Bits that don't divide 32 (3, 5, 6) leave the top bits of each word unused.
const fn per_word(bits: u8) -> usize {
    32 / bits as usize
}

const fn word_count(bits: u8) -> usize {
    VOLUME.div_ceil(per_word(bits))
}

pub(crate) const fn index(x: i32, y: i32, z: i32) -> usize {
    (((x & 15) << 8) | ((z & 15) << 4) | (y & 15)) as usize
}

impl Storage {
    /// Network format: `u8 (bits << 1) | 1`, packed u32 LE words, zigzag varint palette length (absent
    /// when bits = 0), zigzag varint runtime ids.
    pub(crate) fn decode(r: &mut Reader) -> Result<Self, Error> {
        let header = r.u8()?;
        if header & 1 == 0 {
            return Err(Error::PersistentPalette);
        }
        let bits = header >> 1;
        if bits == 0 {
            return Ok(Storage::Single(r.var_i32()? as u32));
        }
        if !VALID_BITS.contains(&bits) {
            return Err(Error::BitsPerBlock(bits));
        }
        let raw = r.bytes(word_count(bits) * 4)?;
        let words = raw.as_chunks::<4>().0.iter().map(|w| u32::from_le_bytes(*w)).collect();
        let len = r.var_i32()?;
        if len <= 0 || len as usize > VOLUME {
            return Err(Error::PaletteLength(len));
        }
        let palette = (0..len).map(|_| r.var_i32().map(|v| v as u32)).collect::<Result<_, _>>()?;
        Ok(Storage::Packed { bits, palette, words })
    }

    /// Advances past a storage whose header byte was already read, without decoding it.
    pub(crate) fn skip(r: &mut Reader, header: u8) -> Result<(), Error> {
        if header & 1 == 0 {
            return Err(Error::PersistentPalette);
        }
        let bits = header >> 1;
        if bits == 0 {
            r.var_u32()?;
            return Ok(());
        }
        if !VALID_BITS.contains(&bits) {
            return Err(Error::BitsPerBlock(bits));
        }
        r.bytes(word_count(bits) * 4)?;
        let len = r.var_i32()?;
        if len <= 0 || len as usize > VOLUME {
            return Err(Error::PaletteLength(len));
        }
        (0..len).try_for_each(|_| r.var_u32().map(drop))
    }

    pub(crate) fn get(&self, idx: usize) -> u32 {
        match self {
            Storage::Single(id) => *id,
            Storage::Packed { bits, palette, words } => {
                let p = packed_get(words, *bits, idx);
                // A malformed index falls back to the first entry instead of panicking.
                palette.get(p).copied().unwrap_or(palette[0])
            }
        }
    }

    pub(crate) fn set(&mut self, idx: usize, id: u32) {
        if let Storage::Single(cur) = *self {
            if cur == id {
                return;
            }
            *self = Storage::Packed { bits: 1, palette: vec![cur], words: vec![0; word_count(1)].into() };
        }
        let Storage::Packed { bits, palette, words } = self else { unreachable!() };
        let p = match palette.iter().position(|&v| v == id) {
            Some(p) => p,
            None => {
                if palette.len() >= 1 << *bits {
                    let grown = *VALID_BITS.iter().find(|&&b| b > *bits).expect("16 bits holds 4096");
                    *words = repack(words, *bits, grown);
                    *bits = grown;
                }
                palette.push(id);
                palette.len() - 1
            }
        };
        packed_set(words, *bits, idx, p);
    }

    pub(crate) fn map_ids(&mut self, f: &dyn Fn(u32) -> u32) {
        match self {
            Storage::Single(id) => *id = f(*id),
            Storage::Packed { palette, .. } => palette.iter_mut().for_each(|v| *v = f(*v)),
        }
    }

    pub(crate) fn copy_into(&self, out: &mut [u32; VOLUME]) {
        match self {
            Storage::Single(id) => out.fill(*id),
            Storage::Packed { bits, palette, words } => {
                let (per, mask) = (per_word(*bits), (1u32 << bits) - 1);
                for (chunk, &w) in out.chunks_mut(per).zip(words.iter()) {
                    for (i, v) in chunk.iter_mut().enumerate() {
                        let p = (w >> (i * *bits as usize)) & mask;
                        *v = palette.get(p as usize).copied().unwrap_or(palette[0]);
                    }
                }
            }
        }
    }

    pub(crate) fn single(&self) -> Option<u32> {
        match self {
            Storage::Single(id) => Some(*id),
            Storage::Packed { .. } => None,
        }
    }

    /// Distinct runtime ids this storage can contain.
    pub(crate) fn palette(&self) -> &[u32] {
        match self {
            Storage::Single(id) => std::slice::from_ref(id),
            Storage::Packed { palette, .. } => palette,
        }
    }
}

fn packed_get(words: &[u32], bits: u8, idx: usize) -> usize {
    let per = per_word(bits);
    let shift = (idx % per) * bits as usize;
    ((words[idx / per] >> shift) & ((1u32 << bits) - 1)) as usize
}

fn packed_set(words: &mut [u32], bits: u8, idx: usize, value: usize) {
    let per = per_word(bits);
    let shift = (idx % per) * bits as usize;
    let mask = ((1u32 << bits) - 1) << shift;
    let w = &mut words[idx / per];
    *w = (*w & !mask) | ((value as u32) << shift);
}

fn repack(words: &[u32], from: u8, to: u8) -> Box<[u32]> {
    let mut out = vec![0; word_count(to)].into_boxed_slice();
    for i in 0..VOLUME {
        packed_set(&mut out, to, i, packed_get(words, from, i));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_grows_palette_and_keeps_values() {
        let mut s = Storage::Single(7);
        for i in 0..VOLUME {
            s.set(i, (i % 40) as u32 + 100);
        }
        let Storage::Packed { bits, .. } = &s else { panic!() };
        assert_eq!(*bits, 6);
        for i in 0..VOLUME {
            assert_eq!(s.get(i), (i % 40) as u32 + 100);
        }
        let mut out = [0; VOLUME];
        s.copy_into(&mut out);
        assert!(out.iter().enumerate().all(|(i, &v)| v == (i % 40) as u32 + 100));
    }
}
