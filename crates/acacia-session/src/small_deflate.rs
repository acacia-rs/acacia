//! Raw deflate for batches too small to repay zlib's state reset, which zeroes a 128 KiB hash table:
//! one fixed-Huffman block of greedy LZ77 matches found with a table sized for the input.

use bytes::{BufMut, BytesMut};

/// Largest input taken here: past it zlib's dynamic Huffman codes start winning on size, and
/// past ~700 bytes on time too (`examples/deflate_cost.rs`).
pub const MAX_INPUT: usize = 256;
const HASH_BITS: u32 = 10;
const MIN_MATCH: usize = 3;
const MAX_MATCH: usize = 258;
const END_OF_BLOCK: u32 = 256;

const LENGTH_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LENGTH_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] =
    [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DIST_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];

struct Bits<'a> {
    out: &'a mut BytesMut,
    acc: u64,
    n: u32,
}

/// The fixed literal/length code (RFC 1951 3.2.6), bit-reversed as it is written, and its length.
const FIXED: [(u16, u8); 288] = {
    let mut table = [(0u16, 0u8); 288];
    let mut s = 0;
    while s < 288 {
        let (code, len) = match s {
            0..=143 => (0x30 + s, 8),
            144..=255 => (0x190 + s - 144, 9),
            256..=279 => (s - 256, 7),
            _ => (0xc0 + s - 280, 8),
        };
        table[s as usize] = (((code as u32).reverse_bits() >> (32 - len)) as u16, len as u8);
        s += 1;
    }
    table
};

impl Bits<'_> {
    /// At most 32 bits at a time; whole bytes are written 4 at a time.
    fn put(&mut self, value: u32, len: u32) {
        self.acc |= u64::from(value) << self.n;
        self.n += len;
        if self.n >= 32 {
            self.out.put_u32_le(self.acc as u32);
            self.acc >>= 32;
            self.n -= 32;
        }
    }

    /// Huffman codes go most significant bit first, unlike every other field.
    fn code(&mut self, code: u32, len: u32) {
        self.put(code.reverse_bits() >> (32 - len), len);
    }

    fn symbol(&mut self, s: u32) {
        let (code, len) = FIXED[s as usize];
        self.put(code.into(), len.into());
    }

    fn back_reference(&mut self, len: usize, dist: usize) {
        let l = LENGTH_BASE.partition_point(|&b| usize::from(b) <= len) - 1;
        self.symbol(257 + l as u32);
        self.put((len - usize::from(LENGTH_BASE[l])) as u32, LENGTH_EXTRA[l].into());
        let d = DIST_BASE.partition_point(|&b| usize::from(b) <= dist) - 1;
        self.code(d as u32, 5);
        self.put((dist - usize::from(DIST_BASE[d])) as u32, DIST_EXTRA[d].into());
    }

    fn finish(self) {
        let bytes = self.n.div_ceil(8) as usize;
        self.out.put_slice(&self.acc.to_le_bytes()[..bytes]);
    }
}

fn hash(at: &[u8]) -> usize {
    (u32::from_le_bytes([at[0], at[1], at[2], 0]).wrapping_mul(2_654_435_761) >> (32 - HASH_BITS)) as usize
}

/// Appends `input` (at most [`MAX_INPUT`] bytes) to `out` as one final raw deflate block.
pub fn compress(input: &[u8], out: &mut BytesMut) {
    debug_assert!(input.len() <= MAX_INPUT);
    // Literals take at most 9 bits.
    out.reserve(input.len() * 9 / 8 + 8);
    let mut bits = Bits { out, acc: 0, n: 0 };
    // BFINAL = 1, BTYPE = 01 (fixed Huffman).
    bits.put(0b011, 3);
    // Positions plus one; 0 is empty. MAX_INPUT fits a u16 and the 32 KiB window.
    let mut head = [0u16; 1 << HASH_BITS];
    let last_hash = input.len().saturating_sub(MIN_MATCH - 1);
    let mut i = 0;
    while i < input.len() {
        if i < last_hash {
            let h = hash(&input[i..]);
            let candidate = usize::from(head[h]);
            head[h] = (i + 1) as u16;
            if let Some(from) = candidate.checked_sub(1) {
                let len = input[from..].iter().zip(&input[i..]).take(MAX_MATCH).take_while(|(a, b)| a == b).count();
                if len >= MIN_MATCH {
                    bits.back_reference(len, i - from);
                    for j in i + 1..(i + len).min(last_hash) {
                        head[hash(&input[j..])] = (j + 1) as u16;
                    }
                    i += len;
                    continue;
                }
            }
        }
        bits.symbol(input[i].into());
        i += 1;
    }
    bits.symbol(END_OF_BLOCK);
    bits.finish();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inflate(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 * MAX_INPUT);
        let status = flate2::Decompress::new(false).decompress_vec(data, &mut out, flate2::FlushDecompress::Finish).unwrap();
        assert_eq!(status, flate2::Status::StreamEnd);
        out
    }

    fn zlib_fast(input: &[u8]) -> usize {
        let mut z = flate2::Compress::new(flate2::Compression::fast(), false);
        let mut out = Vec::with_capacity(input.len() + 64);
        z.compress_vec(input, &mut out, flate2::FlushCompress::Finish).unwrap();
        out.len()
    }

    fn inputs() -> Vec<Vec<u8>> {
        let noise: Vec<u8> = (0..MAX_INPUT as u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
        let text = b"PlayerAuthInput position yaw pitch head_yaw move_vector input_data tick delta ".repeat(14);
        let mut v = vec![Vec::new(), vec![0], vec![1, 2], vec![9; 3], vec![9; 4], vec![0xff; MAX_INPUT], text[..MAX_INPUT].to_vec(), noise.clone()];
        v.extend([MAX_INPUT - 1, 3, 4, 5].map(|n| vec![7; n]));
        v.extend((0..40).map(|n| noise[..n * MAX_INPUT / 40].iter().zip(text.iter()).map(|(a, b)| if n % 3 == 0 { *a } else { a & 3 ^ b }).collect()));
        v
    }

    #[test]
    fn inflates_back() {
        for input in inputs() {
            let mut out = BytesMut::new();
            compress(&input, &mut out);
            assert_eq!(inflate(&out), input, "len {}", input.len());
        }
    }

    #[test]
    fn about_as_small_as_zlib_fast() {
        for input in inputs().into_iter().filter(|i| i.len() >= 64) {
            let mut out = BytesMut::new();
            compress(&input, &mut out);
            let zlib = zlib_fast(&input);
            assert!(out.len() <= zlib + zlib / 8 + 4, "len {}: {} vs zlib {zlib}", input.len(), out.len());
        }
    }
}
