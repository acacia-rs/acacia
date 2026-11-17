//! Length-prefixed packet framing for server code, byte-buffer based so any transport can drive it. A frame is a
//! VarInt length (at most 3 bytes) and its contents; once a compression threshold is set the contents start with
//! the inflated length (0: stored as is). Inflating and deflating need the `zlib` feature; without it only
//! packets below the threshold can be framed and only stored frames read.

use crate::{Error, Wire, read_varint, write_varint};

/// Vanilla's `Varint21LengthFieldPrepender` limit, and its cap on an inflated packet.
pub const MAX_FRAME: usize = (1 << 21) - 1;
pub const MAX_INFLATED: usize = 1 << 23;

/// A packet as sent, before framing: id, then body.
pub fn encode_packet(id: i32, body: &impl Wire) -> Vec<u8> {
    let mut packet = Vec::new();
    write_varint(id, &mut packet);
    body.write(&mut packet);
    packet
}

/// A decoded packet's id and body.
pub fn split_packet(packet: &[u8]) -> Result<(i32, &[u8]), Error> {
    let mut body = packet;
    let id = read_varint(&mut body)?;
    Ok((id, body))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameCodec {
    threshold: Option<usize>,
}

impl FrameCodec {
    pub fn new() -> Self {
        Self::default()
    }

    /// `LoginCompression`'s threshold; a negative one turns compression off.
    pub fn set_compression(&mut self, threshold: i32) {
        self.threshold = usize::try_from(threshold).ok();
    }

    /// The bytes to send for one [`encode_packet`]d packet.
    pub fn frame(&self, packet: &[u8]) -> Result<Vec<u8>, Error> {
        let content = match self.threshold {
            Some(threshold) => compress(packet, threshold)?,
            None => packet.to_vec(),
        };
        if content.len() > MAX_FRAME {
            return Err(Error::Invalid("frame size"));
        }
        let mut out = Vec::with_capacity(content.len() + 3);
        write_varint(content.len() as i32, &mut out);
        out.extend_from_slice(&content);
        Ok(out)
    }

    /// The first complete frame in `buf` as its packet bytes (id, then body) and the bytes it took, or `None`
    /// when more input is needed.
    pub fn unframe(&self, buf: &[u8]) -> Result<Option<(Vec<u8>, usize)>, Error> {
        let Some(prefix) = buf.iter().position(|b| b & 0x80 == 0) else {
            return if buf.len() < 3 { Ok(None) } else { Err(Error::VarIntTooLong) };
        };
        if prefix >= 3 {
            return Err(Error::VarIntTooLong);
        }
        let len = read_varint(&mut &buf[..=prefix])? as usize;
        if len == 0 || len > MAX_FRAME {
            return Err(Error::Invalid("frame size"));
        }
        let end = prefix + 1 + len;
        let Some(content) = buf.get(prefix + 1..end) else { return Ok(None) };
        let packet = if self.threshold.is_some() { inflate(content)? } else { content.to_vec() };
        Ok(Some((packet, end)))
    }
}

fn inflate(content: &[u8]) -> Result<Vec<u8>, Error> {
    let mut input = content;
    let inflated = read_varint(&mut input)?;
    if inflated == 0 {
        return Ok(input.to_vec());
    }
    let inflated = usize::try_from(inflated).ok().filter(|n| *n <= MAX_INFLATED).ok_or(Error::Invalid("inflated size"))?;
    let out = zlib::inflate(input, inflated)?;
    if out.len() != inflated {
        return Err(Error::Invalid("inflated size"));
    }
    Ok(out)
}

fn compress(packet: &[u8], threshold: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::with_capacity(packet.len() + 1);
    if packet.len() < threshold {
        out.push(0);
        out.extend_from_slice(packet);
        return Ok(out);
    }
    write_varint(packet.len() as i32, &mut out);
    zlib::deflate(packet, &mut out)?;
    Ok(out)
}

#[cfg(feature = "zlib")]
mod zlib {
    use std::io::{Read, Write};

    use crate::Error;

    pub fn inflate(input: &[u8], expected: usize) -> Result<Vec<u8>, Error> {
        let mut out = Vec::with_capacity(expected);
        // One byte past the declared length, so an oversized stream shows as a length mismatch.
        flate2::read::ZlibDecoder::new(input).take(expected as u64 + 1).read_to_end(&mut out).map_err(|_| Error::Invalid("zlib stream"))?;
        Ok(out)
    }

    pub fn deflate(packet: &[u8], out: &mut Vec<u8>) -> Result<(), Error> {
        let mut encoder = flate2::write::ZlibEncoder::new(std::mem::take(out), flate2::Compression::default());
        encoder.write_all(packet).map_err(|_| Error::Invalid("zlib stream"))?;
        *out = encoder.finish().map_err(|_| Error::Invalid("zlib stream"))?;
        Ok(())
    }
}

#[cfg(not(feature = "zlib"))]
mod zlib {
    use crate::Error;

    pub fn inflate(_: &[u8], _: usize) -> Result<Vec<u8>, Error> {
        Err(Error::Invalid("zlib feature"))
    }

    pub fn deflate(_: &[u8], _: &mut Vec<u8>) -> Result<(), Error> {
        Err(Error::Invalid("zlib feature"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packets::KeepAlive;

    fn compressing(threshold: i32) -> FrameCodec {
        let mut codec = FrameCodec::new();
        codec.set_compression(threshold);
        codec
    }

    #[test]
    fn plain_frames_round_trip_and_wait_for_more_input() {
        let codec = FrameCodec::new();
        let packet = encode_packet(4, &KeepAlive { id: 9 });
        let framed = codec.frame(&packet).unwrap();
        assert_eq!(framed[0] as usize, packet.len());
        assert_eq!(codec.unframe(&framed).unwrap(), Some((packet.clone(), framed.len())));
        assert_eq!(codec.unframe(&framed[..framed.len() - 1]).unwrap(), None);
        assert_eq!(codec.unframe(&[]).unwrap(), None);
        let (id, body) = split_packet(&packet).unwrap();
        assert_eq!((id, KeepAlive::read(&mut &body[..]).unwrap()), (4, KeepAlive { id: 9 }));
    }

    #[test]
    fn unframe_takes_one_frame_of_several() {
        let codec = FrameCodec::new();
        let mut stream = codec.frame(&[1, 2]).unwrap();
        stream.extend(codec.frame(&[3]).unwrap());
        assert_eq!(codec.unframe(&stream).unwrap(), Some((vec![1, 2], 3)));
        assert_eq!(codec.unframe(&stream[3..]).unwrap(), Some((vec![3], 2)));
    }

    #[test]
    fn bad_lengths_are_rejected() {
        let codec = FrameCodec::new();
        assert!(codec.unframe(&[0]).is_err());
        assert!(codec.unframe(&[0x80, 0x80, 0x80, 0x01]).is_err());
        assert!(codec.unframe(&[0xff, 0xff, 0xff]).is_err());
    }

    #[test]
    fn small_packets_are_stored_under_a_threshold() {
        let codec = compressing(256);
        let framed = codec.frame(&[7; 10]).unwrap();
        assert_eq!(framed[..2], [11, 0]);
        assert_eq!(codec.unframe(&framed).unwrap(), Some((vec![7; 10], framed.len())));
        assert!(compressing(-1).frame(&[7; 300]).unwrap().len() == 302, "negative threshold is off");
    }

    #[cfg(feature = "zlib")]
    #[test]
    fn compressed_frames_round_trip() {
        let codec = compressing(256);
        for size in [255, 256, 70_000] {
            let packet = vec![7u8; size];
            let framed = codec.frame(&packet).unwrap();
            assert_eq!(codec.unframe(&framed).unwrap(), Some((packet, framed.len())));
        }
        let mut framed = codec.frame(&[7; 1000]).unwrap();
        framed[1] = 0x81;
        assert!(codec.unframe(&framed).is_err(), "wrong declared length");
    }

    #[cfg(not(feature = "zlib"))]
    #[test]
    fn large_packets_need_the_zlib_feature() {
        assert!(compressing(256).frame(&[7; 300]).is_err());
    }
}
