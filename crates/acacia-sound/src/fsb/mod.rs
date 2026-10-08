//! FMOD FSB5 sound banks as the Bedrock pack ships them (one sample each): the container in
//! container.rs, the FADPCM codec in fadpcm.rs. PCM16 is decoded inline.

pub mod container;
mod fadpcm;
#[cfg(test)]
mod tests;

use std::fmt;

pub use container::{Sample, parse};

/// `FMOD_SOUND_FORMAT_PCM16`.
pub const CODEC_PCM16: u32 = 0x02;
/// `FMOD_SOUND_FORMAT_FADPCM`.
pub const CODEC_FADPCM: u32 = 0x10;

/// Decoded audio: interleaved signed 16-bit samples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pcm {
    pub sample_rate: u32,
    pub channels: u16,
    /// `frames * channels` samples, channel-interleaved.
    pub samples: Vec<i16>,
}

/// Why an FSB5 file couldn't be decoded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The file doesn't start with `FSB5`.
    NotFsb5,
    /// An FSB5 header version other than 0 or 1.
    Version(u32),
    /// The file ends inside a header.
    Truncated,
    /// The header is inconsistent.
    Malformed(&'static str),
    /// The bank holds no samples.
    Empty,
    /// A codec this decoder doesn't handle (an `FMOD_SOUND_FORMAT_*` value).
    Unsupported(u32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFsb5 => f.write_str("not an FSB5 file"),
            Error::Version(v) => write!(f, "unsupported FSB5 version {v}"),
            Error::Truncated => f.write_str("FSB5 file is truncated"),
            Error::Malformed(why) => write!(f, "malformed FSB5 file: {why}"),
            Error::Empty => f.write_str("FSB5 bank holds no samples"),
            Error::Unsupported(c) => write!(f, "unsupported FSB5 codec {} ({c:#x})", codec_name(*c)),
        }
    }
}

impl std::error::Error for Error {}

/// Decodes the first sample of an FSB5 bank to PCM.
pub fn decode(bytes: &[u8]) -> Result<Pcm, Error> {
    let sample = parse(bytes)?.into_iter().next().ok_or(Error::Empty)?;
    decode_sample(&sample)
}

/// Decodes one parsed sample to PCM.
pub fn decode_sample(sample: &Sample<'_>) -> Result<Pcm, Error> {
    let channels = usize::from(sample.channels);
    let frames = sample.num_samples as usize;
    let samples = match sample.codec {
        CODEC_FADPCM => fadpcm::decode(sample.data, channels, frames),
        CODEC_PCM16 => pcm16(sample.data, frames * channels, sample.flags & container::FLAG_BIG_ENDIAN != 0),
        c => return Err(Error::Unsupported(c)),
    };
    Ok(Pcm { sample_rate: sample.sample_rate, channels: sample.channels, samples })
}

/// `len` samples of 16-bit PCM; missing data decodes as silence.
fn pcm16(data: &[u8], len: usize, big_endian: bool) -> Vec<i16> {
    let read = if big_endian { i16::from_be_bytes } else { i16::from_le_bytes };
    let mut out: Vec<i16> = data.as_chunks::<2>().0.iter().take(len).map(|&b| read(b)).collect();
    out.resize(len, 0);
    out
}

fn codec_name(codec: u32) -> &'static str {
    const NAMES: [&str; 18] = [
        "NONE", "PCM8", "PCM16", "PCM24", "PCM32", "PCMFLOAT", "GCADPCM", "IMAADPCM", "VAG", "HEVAG", "XMA",
        "MPEG", "CELT", "AT9", "XWMA", "VORBIS", "FADPCM", "OPUS",
    ];
    NAMES.get(codec as usize).copied().unwrap_or("unknown")
}
