use bytes::{BufMut, Bytes, BytesMut};

use crate::compression::{decompress_batch, Algorithm, NONE_ID};
use crate::crypto::Encryption;
use crate::Error;

pub const GAME_MESSAGE_ID: u8 = 0xfe;
const DEFAULT_MAX_DECOMPRESSED: usize = 16 * 1024 * 1024;

/// Frames packets into 0xfe game messages: `[0xfe][compression id][varint len + packet]*`, with
/// everything after 0xfe encrypted once the handshake completes.
pub struct BatchCodec {
    header: bool,
    compression: Option<(Algorithm, usize)>,
    encryption: Option<Encryption>,
    max_decompressed: usize,
}

impl Default for BatchCodec {
    fn default() -> Self {
        Self { header: true, compression: None, encryption: None, max_decompressed: DEFAULT_MAX_DECOMPRESSED }
    }
}

impl BatchCodec {
    /// NetherNet framing: identical batches with no 0xfe byte in front.
    pub fn without_header() -> Self {
        Self { header: false, ..Self::default() }
    }

    pub fn enable_compression(&mut self, alg: Algorithm, threshold: usize) {
        self.compression = Some((alg, threshold));
    }

    pub fn enable_encryption(&mut self, key: [u8; 32]) {
        self.encryption = Some(Encryption::new(key));
    }

    pub fn is_encrypted(&self) -> bool {
        self.encryption.is_some()
    }

    /// Encodes already-serialized packets (header + body) into one game message.
    pub fn encode<'a>(&mut self, packets: impl IntoIterator<Item = &'a [u8]>) -> Bytes {
        let mut plain = BytesMut::new();
        for p in packets {
            put_varuint32(&mut plain, p.len() as u32);
            plain.put_slice(p);
        }
        let mut out = BytesMut::with_capacity(plain.len() + 16);
        if self.header {
            out.put_u8(GAME_MESSAGE_ID);
        }
        match self.compression {
            Some((alg, threshold)) if plain.len() >= threshold => {
                out.put_u8(alg.batch_id());
                alg.compress(&plain, &mut out);
            }
            Some(_) => {
                out.put_u8(NONE_ID);
                out.put_slice(&plain);
            }
            None => out.put_slice(&plain),
        }
        if let Some(enc) = &mut self.encryption {
            enc.seal(&mut out, usize::from(self.header));
        }
        out.freeze()
    }

    /// Decodes a game message, appending each packet (zero-copy slices of the batch) to `out`.
    pub fn decode(&mut self, msg: &[u8], out: &mut Vec<Bytes>) -> Result<(), Error> {
        let body = if self.header {
            let (&id, body) = msg.split_first().ok_or(Error::EmptyBatch)?;
            if id != GAME_MESSAGE_ID {
                return Err(Error::NotGameMessage(id));
            }
            body
        } else if msg.is_empty() {
            return Err(Error::EmptyBatch);
        } else {
            msg
        };
        let mut body = body.to_vec();
        if let Some(enc) = &mut self.encryption {
            let len = enc.open(&mut body)?;
            body.truncate(len);
        }
        let payload = if self.compression.is_some() {
            match decompress_batch(&body, self.max_decompressed)? {
                Some(decompressed) => Bytes::from(decompressed),
                None => Bytes::from(body).slice(1..),
            }
        } else {
            Bytes::from(body)
        };
        split_packets(&payload, out)
    }
}

fn split_packets(payload: &Bytes, out: &mut Vec<Bytes>) -> Result<(), Error> {
    let mut pos = 0;
    while pos < payload.len() {
        let (len, n) = read_varuint32(&payload[pos..]).ok_or(Error::MalformedBatch)?;
        let start = pos + n;
        let end = start.checked_add(len as usize).filter(|&e| e <= payload.len()).ok_or(Error::MalformedBatch)?;
        out.push(payload.slice(start..end));
        pos = end;
    }
    Ok(())
}

fn put_varuint32(out: &mut BytesMut, mut v: u32) {
    while v >= 0x80 {
        out.put_u8(v as u8 | 0x80);
        v >>= 7;
    }
    out.put_u8(v as u8);
}

fn read_varuint32(buf: &[u8]) -> Option<(u32, usize)> {
    let mut v = 0u32;
    for (i, &b) in buf.iter().take(5).enumerate() {
        v |= u32::from(b & 0x7f) << (7 * i);
        if b & 0x80 == 0 {
            return Some((v, i + 1));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(tx: &mut BatchCodec, rx: &mut BatchCodec, packets: &[&[u8]]) {
        let msg = tx.encode(packets.iter().copied());
        let mut out = vec![];
        rx.decode(&msg, &mut out).unwrap();
        assert_eq!(out.iter().map(|b| &b[..]).collect::<Vec<_>>(), packets);
    }

    #[test]
    fn plain_then_compressed_then_encrypted() {
        let big = vec![9u8; 5000];
        let packets: &[&[u8]] = &[b"\x01abc", &big, b"\x02"];
        let (mut tx, mut rx) = (BatchCodec::default(), BatchCodec::default());
        roundtrip(&mut tx, &mut rx, packets);

        for alg in [Algorithm::Snappy, Algorithm::Deflate] {
            tx.enable_compression(alg, 256);
            rx.enable_compression(alg, 256);
            roundtrip(&mut tx, &mut rx, packets);
            roundtrip(&mut tx, &mut rx, &[b"tiny"]);
        }

        tx.enable_encryption([4; 32]);
        rx.enable_encryption([4; 32]);
        for _ in 0..3 {
            roundtrip(&mut tx, &mut rx, packets);
        }
    }

    #[test]
    fn headerless_matches_framed_minus_first_byte() {
        let packets: &[&[u8]] = &[b"\xc1\x01\x00\x00\x08\x91"];
        let framed = BatchCodec::default().encode(packets.iter().copied());
        let (mut tx, mut rx) = (BatchCodec::without_header(), BatchCodec::without_header());
        assert_eq!(tx.encode(packets.iter().copied()), framed.slice(1..));
        roundtrip(&mut tx, &mut rx, packets);
        tx.enable_compression(Algorithm::Deflate, 0);
        rx.enable_compression(Algorithm::Deflate, 0);
        roundtrip(&mut tx, &mut rx, packets);
    }
}
