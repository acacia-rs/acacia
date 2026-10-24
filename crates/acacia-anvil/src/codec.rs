//! Chunk compression in region slots: Minecraft's type 2 (zlib), 3 (none) and 4 (LZ4 in lz4-java's
//! `LZ4BlockOutputStream` framing, not the LZ4 frame format). Java BlueMap 5.29 and bluemap-rs read all
//! three; reading also takes type 1 (gzip).

use std::io::{self, Read, Write};
use std::str::FromStr;

use flate2::write::ZlibEncoder;

use crate::Error;

const SECTOR: usize = 4096;
/// The offset table stores a sector count in one byte; bigger chunks would need an external `.mcc` file.
const MAX_SECTORS: usize = 255;

const GZIP: u8 = 1;
const ZLIB: u8 = 2;
const NONE: u8 = 3;
const LZ4: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    /// zlib at a level 0–9.
    Zlib(u32),
    Lz4,
    None,
}

impl Default for Compression {
    fn default() -> Self {
        Self::Zlib(6)
    }
}

impl Compression {
    /// Parsed from the environment variable `var` (examples and tools), the default when it is unset.
    pub fn from_env_or_default(var: &str) -> Result<Self, String> {
        std::env::var(var).map_or(Ok(Self::default()), |s| s.parse())
    }
}

impl FromStr for Compression {
    type Err = String;

    /// `zlib` (level 6), `zlib:<0-9>`, `lz4`, `none`.
    fn from_str(s: &str) -> Result<Self, String> {
        match s.split_once(':') {
            None if s == "zlib" => Ok(Self::default()),
            None if s == "lz4" => Ok(Self::Lz4),
            None if s == "none" => Ok(Self::None),
            Some(("zlib", l)) => match l.parse() {
                Ok(l @ 0..=9) => Ok(Self::Zlib(l)),
                _ => Err(format!("zlib level 0-9: {s}")),
            },
            _ => Err(format!("compression zlib[:level], lz4 or none: {s}")),
        }
    }
}

/// A region slot's payload: compression byte followed by the compressed chunk NBT.
pub fn compress(nbt: &[u8], c: Compression) -> Result<Vec<u8>, Error> {
    let payload = match c {
        Compression::Zlib(level) => {
            let mut payload = Vec::with_capacity(nbt.len() / 4 + 16);
            payload.push(ZLIB);
            let mut enc = ZlibEncoder::new(payload, flate2::Compression::new(level));
            enc.write_all(nbt).expect("writing to a Vec");
            enc.finish().expect("writing to a Vec")
        }
        Compression::Lz4 => {
            let mut payload = vec![LZ4];
            lz4_block::compress_into(nbt, &mut payload);
            payload
        }
        Compression::None => [&[NONE][..], nbt].concat(),
    };
    if (payload.len() + 4).div_ceil(SECTOR) > MAX_SECTORS {
        return Err(Error::ChunkTooLarge(payload.len()));
    }
    Ok(payload)
}

/// Decompresses a stored chunk of any type [`compress`] writes, or gzip.
pub fn decode_payload(payload: &[u8]) -> io::Result<Vec<u8>> {
    let (&ty, data) = payload
        .split_first()
        .ok_or_else(|| invalid("empty chunk payload"))?;
    let mut out = Vec::new();
    match ty {
        ZLIB => {
            flate2::read::ZlibDecoder::new(data).read_to_end(&mut out)?;
        }
        GZIP => {
            flate2::read::GzDecoder::new(data).read_to_end(&mut out)?;
        }
        NONE => out.extend_from_slice(data),
        LZ4 => lz4_block::decompress_into(data, &mut out)?,
        _ => return Err(invalid("unknown chunk compression")),
    }
    Ok(out)
}

fn invalid(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// lz4-java's block stream: blocks of "LZ4Block", token (method | log2(block size) - 10), compressed and
/// original length and checksum (i32 LE), then the data; an empty block ends the stream.
/// <https://github.com/lz4/lz4-java/blob/master/src/java/net/jpountz/lz4/LZ4BlockOutputStream.java>
pub mod lz4_block {
    use std::io;

    use twox_hash::XxHash32;

    use super::invalid;

    const MAGIC: &[u8; 8] = b"LZ4Block";
    const HEADER: usize = MAGIC.len() + 13;
    const RAW: u8 = 0x10;
    const LZ4: u8 = 0x20;
    /// lz4-java's default block size, 64 KiB.
    const LEVEL: u8 = 6;
    const BLOCK: usize = 1 << (10 + LEVEL);

    /// lz4-java's `StreamingXXHash32.asChecksum()` (seed `0x9747b28c`) keeps the low 28 bits.
    fn checksum(block: &[u8]) -> u32 {
        XxHash32::oneshot(0x9747_b28c, block) & 0x0FFF_FFFF
    }

    fn header(out: &mut [u8], method: u8, compressed: usize, original: usize, check: u32) {
        out[..8].copy_from_slice(MAGIC);
        out[8] = method | LEVEL;
        for (i, v) in [compressed as u32, original as u32, check].into_iter().enumerate() {
            out[9 + 4 * i..13 + 4 * i].copy_from_slice(&v.to_le_bytes());
        }
    }

    /// Appends what lz4-java writes: 64 KiB blocks, each LZ4 unless that does not shrink it, then the end
    /// block.
    pub fn compress_into(data: &[u8], out: &mut Vec<u8>) {
        for block in data.chunks(BLOCK) {
            let at = out.len();
            out.resize(at + HEADER + lz4_flex::block::get_maximum_output_size(block.len()), 0);
            let n = lz4_flex::block::compress_into(block, &mut out[at + HEADER..])
                .expect("output sized to the bound");
            let (method, n) = if n < block.len() {
                (LZ4, n)
            } else {
                out[at + HEADER..at + HEADER + block.len()].copy_from_slice(block);
                (RAW, block.len())
            };
            header(&mut out[at..], method, n, block.len(), checksum(block));
            out.truncate(at + HEADER + n);
        }
        let at = out.len();
        out.resize(at + HEADER, 0);
        header(&mut out[at..], RAW, 0, 0, 0);
    }

    /// Appends the blocks up to the end block, checking each block's checksum.
    pub fn decompress_into(mut data: &[u8], out: &mut Vec<u8>) -> io::Result<()> {
        loop {
            if data.len() < HEADER || data[..8] != *MAGIC {
                return Err(invalid("lz4 block header"));
            }
            let le = |i: usize| u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as usize;
            let (method, compressed, original, check) = (data[8] & 0xF0, le(9), le(13), le(17));
            if compressed == 0 && original == 0 {
                return Ok(());
            }
            let payload = data
                .get(HEADER..HEADER + compressed)
                .filter(|_| original <= BLOCK)
                .ok_or_else(|| invalid("lz4 block length"))?;
            let start = out.len();
            match method {
                RAW if compressed == original => out.extend_from_slice(payload),
                LZ4 => {
                    out.resize(start + original, 0);
                    let n = lz4_flex::block::decompress_into(payload, &mut out[start..])
                        .map_err(|_| invalid("lz4 block data"))?;
                    if n != original {
                        return Err(invalid("lz4 block length"));
                    }
                }
                _ => return Err(invalid("lz4 block method")),
            }
            if checksum(&out[start..]) as usize != check {
                return Err(invalid("lz4 block checksum"));
            }
            data = &data[HEADER + compressed..];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_compression_round_trips() {
        let mut big: Vec<u8> = (0..200_000u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
            .collect();
        big[70_000..140_000].fill(15);
        for c in ["zlib", "zlib:1", "lz4", "none"] {
            let c: Compression = c.parse().unwrap();
            for data in [&b""[..], b"hello", &big] {
                let payload = compress(data, c).unwrap();
                assert_eq!(decode_payload(&payload).unwrap(), data, "{c:?}");
            }
        }
    }

    /// The lz4-java stream `LZ4BlockOutputStream` writes for "hello" (one raw block: LZ4 does not shrink it).
    #[test]
    fn lz4_matches_lz4_java_framing() {
        let payload = compress(b"hello", Compression::Lz4).unwrap();
        let mut want = vec![LZ4];
        want.extend_from_slice(b"LZ4Block\x16\x05\0\0\0\x05\0\0\0");
        want.extend_from_slice(&lz4_checksum_of_hello().to_le_bytes());
        want.extend_from_slice(b"hello");
        want.extend_from_slice(b"LZ4Block\x16\0\0\0\0\0\0\0\0\0\0\0\0");
        assert_eq!(payload, want);
    }

    fn lz4_checksum_of_hello() -> u32 {
        twox_hash::XxHash32::oneshot(0x9747_b28c, b"hello") & 0x0FFF_FFFF
    }

    #[test]
    fn a_torn_lz4_stream_is_an_error() {
        let nbt = vec![7u8; 100_000];
        let payload = compress(&nbt, Compression::Lz4).unwrap();
        for cut in [1, 20, payload.len() / 2, payload.len() - 1] {
            assert!(decode_payload(&payload[..cut]).is_err(), "cut at {cut}");
        }
    }
}
