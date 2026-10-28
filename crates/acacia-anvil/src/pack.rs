//! Java's paletted-container long packing (1.16+): `bits` per entry, entries never span two longs.

/// Bits per block-state index: at least 4, as Java's `PalettedContainer` strategy for sections.
pub fn block_bits(palette_len: usize) -> u32 {
    ceil_log2(palette_len).max(4)
}

/// Bits per biome index: no minimum (a one-entry palette writes no data at all).
pub fn biome_bits(palette_len: usize) -> u32 {
    ceil_log2(palette_len).max(1)
}

fn ceil_log2(n: usize) -> u32 {
    if n <= 1 {
        0
    } else {
        usize::BITS - (n - 1).leading_zeros()
    }
}

pub fn pack_into(indices: &[u16], bits: u32, out: &mut Vec<u64>) {
    out.clear();
    let per_long = (64 / bits) as usize;
    out.reserve(indices.len().div_ceil(per_long));
    for chunk in indices.chunks(per_long) {
        let mut v = 0u64;
        for (i, &idx) in chunk.iter().enumerate() {
            v |= (idx as u64) << (i as u32 * bits);
        }
        out.push(v);
    }
}

/// The inverse of [`pack_into`]: fills `out` from `longs`; false if `longs` is too short.
pub fn unpack(longs: &[u64], bits: u32, out: &mut [u16]) -> bool {
    let per_long = (64 / bits) as usize;
    if longs.len() < out.len().div_ceil(per_long) {
        return false;
    }
    let mask = (1u64 << bits) - 1;
    for (i, o) in out.iter_mut().enumerate() {
        *o = ((longs[i / per_long] >> ((i % per_long) as u32 * bits)) & mask) as u16;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpack_inverts_pack() {
        let idx: Vec<u16> = (0..100).map(|i| i % 23).collect();
        let mut longs = Vec::new();
        pack_into(&idx, 5, &mut longs);
        let mut back = vec![0; 100];
        assert!(unpack(&longs, 5, &mut back));
        assert_eq!(back, idx);
    }

    #[test]
    fn bits() {
        assert_eq!(block_bits(1), 4);
        assert_eq!(block_bits(17), 5);
        assert_eq!(biome_bits(2), 1);
        assert_eq!(biome_bits(5), 3);
    }

    #[test]
    fn five_bits_leave_padding() {
        let idx: Vec<u16> = (0..13).collect();
        let mut out = Vec::new();
        pack_into(&idx, 5, &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(out[1], 12);
        assert_eq!(out[0] & 0x1F, 0);
        assert_eq!((out[0] >> 55) & 0x1F, 11);
        assert_eq!(out[0] >> 60, 0);
    }
}
