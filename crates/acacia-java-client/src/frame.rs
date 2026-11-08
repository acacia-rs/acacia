use std::io::{Read, Write};

use acacia_java_proto::{Wire, read_varint, write_varint};
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

/// Vanilla's `Varint21LengthFieldPrepender` limit, and its cap on an inflated packet.
const MAX_FRAME: usize = (1 << 21) - 1;
const MAX_INFLATED: usize = 1 << 23;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Decode(#[from] acacia_java_proto::Error),
    #[error("frame of {0} bytes is out of range")]
    BadFrame(usize),
    #[error("disconnected: {0}")]
    Disconnected(String),
    #[error("packet {0:#x} is not expected during {1}")]
    Unexpected(i32, &'static str),
}

/// Length-prefixed frames over TCP, zlib-compressed once the server sets a threshold.
pub struct Connection {
    stream: BufReader<TcpStream>,
    threshold: Option<usize>,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream: BufReader::new(stream), threshold: None }
    }

    pub fn set_compression(&mut self, threshold: i32) {
        self.threshold = usize::try_from(threshold).ok();
    }

    async fn read_frame_length(&mut self) -> Result<usize, Error> {
        let mut bytes = [0u8; 3];
        for i in 0..bytes.len() {
            bytes[i] = self.stream.read_u8().await?;
            if bytes[i] & 0x80 == 0 {
                return Ok(read_varint(&mut &bytes[..=i])? as usize);
            }
        }
        Err(Error::BadFrame(MAX_FRAME + 1))
    }

    /// The next packet's id and body.
    pub async fn read(&mut self) -> Result<(i32, Vec<u8>), Error> {
        let len = self.read_frame_length().await?;
        if len == 0 || len > MAX_FRAME {
            return Err(Error::BadFrame(len));
        }
        let mut frame = vec![0; len];
        self.stream.read_exact(&mut frame).await?;
        let mut data = if self.threshold.is_some() { inflate(&frame)? } else { frame };
        let mut body = &data[..];
        let id = read_varint(&mut body)?;
        let start = data.len() - body.len();
        data.drain(..start);
        Ok((id, data))
    }

    pub async fn send(&mut self, id: i32, body: &impl Wire) -> Result<(), Error> {
        let mut packet = Vec::new();
        write_varint(id, &mut packet);
        body.write(&mut packet);
        let content = match self.threshold {
            Some(threshold) => compress(packet, threshold)?,
            None => packet,
        };
        let mut frame = Vec::with_capacity(content.len() + 3);
        write_varint(content.len() as i32, &mut frame);
        frame.extend_from_slice(&content);
        self.stream.write_all(&frame).await?;
        Ok(())
    }
}

/// A compressed frame's contents: inflated length (0: stored as is), then the packet.
fn inflate(frame: &[u8]) -> Result<Vec<u8>, Error> {
    let mut input = frame;
    let inflated = read_varint(&mut input)? as usize;
    if inflated == 0 {
        return Ok(input.to_vec());
    }
    if inflated > MAX_INFLATED {
        return Err(Error::BadFrame(inflated));
    }
    let mut out = Vec::with_capacity(inflated);
    ZlibDecoder::new(input).take(inflated as u64 + 1).read_to_end(&mut out)?;
    if out.len() != inflated {
        return Err(Error::BadFrame(out.len()));
    }
    Ok(out)
}

fn compress(packet: Vec<u8>, threshold: usize) -> Result<Vec<u8>, Error> {
    let mut out = Vec::with_capacity(packet.len() + 1);
    if packet.len() < threshold {
        out.push(0);
        out.extend_from_slice(&packet);
        return Ok(out);
    }
    write_varint(packet.len() as i32, &mut out);
    let mut encoder = ZlibEncoder::new(out, Compression::default());
    encoder.write_all(&packet)?;
    Ok(encoder.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressed_frames_round_trip() {
        for size in [1, 255, 256, 70_000] {
            let packet = vec![7u8; size];
            assert_eq!(inflate(&compress(packet.clone(), 256).unwrap()).unwrap(), packet);
        }
    }

    #[test]
    fn inflate_rejects_a_wrong_declared_length() {
        let mut frame = compress(vec![7; 1000], 256).unwrap();
        frame[0] = 0x81;
        assert!(matches!(inflate(&frame), Err(Error::BadFrame(_))));
    }
}
