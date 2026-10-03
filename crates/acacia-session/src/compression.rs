use std::io::{Read, Write};

use acacia_proto::packets::NetworkSettingsCompressionAlgorithm;
use bytes::BytesMut;

use crate::Error;

/// Batch header byte meaning "not compressed" (used below the server's compression threshold).
pub const NONE_ID: u8 = 0xff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// Raw deflate (no zlib header).
    Deflate,
    /// Snappy block format.
    Snappy,
}

impl Algorithm {
    /// Maps the NetworkSettings `compression_algorithm` field.
    pub fn from_network_id(id: u16) -> Option<Self> {
        match id {
            0 => Some(Self::Deflate),
            1 => Some(Self::Snappy),
            _ => None,
        }
    }

    pub fn from_settings(alg: NetworkSettingsCompressionAlgorithm) -> Result<Self, Error> {
        match alg {
            NetworkSettingsCompressionAlgorithm::Deflate => Ok(Self::Deflate),
            NetworkSettingsCompressionAlgorithm::Snappy => Ok(Self::Snappy),
            NetworkSettingsCompressionAlgorithm::Unknown(id) => Err(Error::UnknownCompression(id as u8)),
        }
    }

    pub fn batch_id(self) -> u8 {
        match self {
            Self::Deflate => 0,
            Self::Snappy => 1,
        }
    }

    fn from_batch_id(id: u8) -> Option<Self> {
        Self::from_network_id(id.into())
    }

    pub fn compress(self, input: &[u8], out: &mut BytesMut) {
        match self {
            Self::Deflate => {
                let mut enc = flate2::write::DeflateEncoder::new(Vec::with_capacity(input.len() / 2), flate2::Compression::fast());
                enc.write_all(input).expect("writing to a Vec cannot fail");
                out.extend_from_slice(&enc.finish().expect("writing to a Vec cannot fail"));
            }
            Self::Snappy => {
                let start = out.len();
                out.resize(start + snap::raw::max_compress_len(input.len()), 0);
                let n = snap::raw::Encoder::new().compress(input, &mut out[start..]).expect("buffer sized by max_compress_len");
                out.truncate(start + n);
            }
        }
    }

    pub fn decompress(self, input: &[u8], max_len: usize) -> Result<Vec<u8>, Error> {
        match self {
            Self::Deflate => {
                let mut out = Vec::with_capacity((input.len() * 4).min(max_len));
                flate2::read::DeflateDecoder::new(input)
                    .take(max_len as u64 + 1)
                    .read_to_end(&mut out)
                    .map_err(|e| Error::Decompress(e.to_string()))?;
                if out.len() > max_len {
                    return Err(Error::BatchTooLarge);
                }
                Ok(out)
            }
            Self::Snappy => {
                let len = snap::raw::decompress_len(input).map_err(|e| Error::Decompress(e.to_string()))?;
                if len > max_len {
                    return Err(Error::BatchTooLarge);
                }
                snap::raw::Decoder::new().decompress_vec(input).map_err(|e| Error::Decompress(e.to_string()))
            }
        }
    }
}

/// Decompresses a batch body whose first byte is the compression ID.
pub fn decompress_batch(body: &[u8], max_len: usize) -> Result<Option<Vec<u8>>, Error> {
    let (&id, rest) = body.split_first().ok_or(Error::EmptyBatch)?;
    if id == NONE_ID {
        return Ok(None);
    }
    let alg = Algorithm::from_batch_id(id).ok_or(Error::UnknownCompression(id))?;
    alg.decompress(rest, max_len).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_both_algorithms() {
        let input: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        for alg in [Algorithm::Deflate, Algorithm::Snappy] {
            let mut buf = BytesMut::from(&[alg.batch_id()][..]);
            alg.compress(&input, &mut buf);
            assert_eq!(decompress_batch(&buf, 1 << 20).unwrap().unwrap(), input);
            assert!(matches!(decompress_batch(&buf, 100), Err(Error::BatchTooLarge)));
        }
    }
}
