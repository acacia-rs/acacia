//! FSB5 header parsing, ported from vgmstream `src/meta/fsb5.c` (`parse_header`).

use super::Error;

/// FSB5 header flag: PCM samples are big-endian.
pub(crate) const FLAG_BIG_ENDIAN: u32 = 0x01;

/// One sample (subsong) of an FSB5 bank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample<'a> {
    /// FMOD sound format (`FMOD_SOUND_FORMAT_*`): 2 is PCM16, 16 is FADPCM.
    pub codec: u32,
    /// Bank-wide flags (version 1 headers only, else 0).
    pub flags: u32,
    pub channels: u16,
    pub sample_rate: u32,
    /// Sample frames (per channel).
    pub num_samples: u32,
    /// Loop start and end (end exclusive), when the header has loop info.
    pub looping: Option<(u32, u32)>,
    /// The sample's name from the bank's name table, if it has one.
    pub name: Option<&'a str>,
    /// The sample's encoded data.
    pub data: &'a [u8],
}

const RATES: [u32; 11] = [4000, 8000, 11000, 11025, 16000, 22050, 24000, 32000, 44100, 48000, 96000];
const CHANNELS: [u16; 4] = [1, 2, 6, 8];

/// Parses an FSB5 bank into its samples, in bank order.
pub fn parse(bytes: &[u8]) -> Result<Vec<Sample<'_>>, Error> {
    if bytes.get(..4) != Some(b"FSB5") {
        return Err(Error::NotFsb5);
    }
    let version = u32_at(bytes, 0x04)?;
    let count = u32_at(bytes, 0x08)? as usize;
    let headers_size = u32_at(bytes, 0x0c)? as usize;
    let names_size = u32_at(bytes, 0x10)? as usize;
    let data_size = u32_at(bytes, 0x14)? as usize;
    let codec = u32_at(bytes, 0x18)?;
    let (flags, base) = match version {
        1 => (u32_at(bytes, 0x20)?, 0x3c),
        0 => (0, 0x40),
        v => return Err(Error::Version(v)),
    };
    let names_at = base + headers_size;
    let data_at = names_at + names_size;
    if data_at + data_size != bytes.len() {
        return Err(Error::Malformed("section sizes don't add up to the file size"));
    }
    let data = &bytes[data_at..];

    let mut offset = base;
    let mut entries = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        let (entry, next) = sample_header(bytes, offset, names_at)?;
        entries.push(entry);
        offset = next;
    }

    let mut samples = Vec::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        let end = entries.get(i + 1).map_or(data_size, |n| n.data_offset);
        if end <= e.data_offset || end > data_size {
            return Err(Error::Malformed("sample data offsets out of order or out of range"));
        }
        let name = if names_size == 0 { None } else { Some(name_at(bytes, names_at, data_at, i)?) };
        samples.push(Sample {
            codec,
            flags,
            channels: e.channels,
            sample_rate: e.sample_rate,
            num_samples: e.num_samples,
            looping: e.looping,
            name,
            data: &data[e.data_offset..end],
        });
    }
    Ok(samples)
}

struct Entry {
    channels: u16,
    sample_rate: u32,
    num_samples: u32,
    looping: Option<(u32, u32)>,
    data_offset: usize,
}

/// Reads the sample header at `offset`; returns it and the offset just past it.
fn sample_header(bytes: &[u8], mut offset: usize, end: usize) -> Result<(Entry, usize), Error> {
    if offset + 8 > end {
        return Err(Error::Truncated);
    }
    let mode = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    offset += 8;
    let mut entry = Entry {
        num_samples: (mode >> 34) as u32 & 0x3fff_ffff,
        data_offset: (((mode >> 7) & 0x07ff_ffff) << 5) as usize,
        channels: CHANNELS[(mode >> 5) as usize & 0x03],
        sample_rate: *RATES.get((mode >> 1) as usize & 0x0f).ok_or(Error::Malformed("sample rate code"))?,
        looping: None,
    };
    let mut more = mode & 1 != 0;
    while more {
        let flag = if offset + 4 <= end { u32_at(bytes, offset)? } else { return Err(Error::Truncated) };
        let kind = (flag >> 25) & 0x7f;
        let size = ((flag >> 1) & 0x00ff_ffff) as usize;
        more = flag & 1 != 0;
        if offset + 4 + size > end {
            return Err(Error::Truncated);
        }
        let body = &bytes[offset + 4..offset + 4 + size];
        match kind {
            0x01 => entry.channels = u16::from(*body.first().ok_or(Error::Truncated)?),
            0x02 => entry.sample_rate = u32_at(body, 0)?,
            // vgmstream's "unwanted loop" heuristic is playback policy, not parsing; callers decide.
            0x03 => {
                let start = u32_at(body, 0)?;
                let end = if size > 4 { u32_at(body, 4)? + 1 } else { entry.num_samples };
                entry.looping = Some((start, end));
            }
            _ => {}
        }
        offset += 4 + size;
    }
    if entry.channels == 0 {
        return Err(Error::Malformed("zero channels"));
    }
    Ok((entry, offset))
}

/// Sample `i`'s name: the name table starts with one u32 offset per sample, relative to the table.
fn name_at(bytes: &[u8], names_at: usize, data_at: usize, i: usize) -> Result<&str, Error> {
    let start = names_at + u32_at(bytes, names_at + 4 * i)? as usize;
    let table = bytes.get(start..data_at).ok_or(Error::Malformed("name offset out of range"))?;
    let len = table.iter().position(|&b| b == 0).unwrap_or(table.len());
    std::str::from_utf8(&table[..len]).map_err(|_| Error::Malformed("sample name isn't UTF-8"))
}

pub(crate) fn u32_at(bytes: &[u8], at: usize) -> Result<u32, Error> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or(Error::Truncated)
}
