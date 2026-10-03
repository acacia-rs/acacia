use bytes::{BufMut, Bytes, BytesMut};

use super::{Reader, Result, WireError, WriteExt, U24_MASK};

pub const FLAG_VALID: u8 = 0x80;
pub const FLAG_ACK: u8 = 0x40;
pub const FLAG_NACK: u8 = 0x20;
pub const FLAG_NEEDS_B_AND_AS: u8 = 0x04;
pub const DATAGRAM_HEADER_LEN: usize = 4;

const FLAG_SPLIT: u8 = 0x10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Reliability {
    Unreliable = 0,
    UnreliableSequenced = 1,
    Reliable = 2,
    ReliableOrdered = 3,
    ReliableSequenced = 4,
    UnreliableWithAck = 5,
    ReliableWithAck = 6,
    ReliableOrderedWithAck = 7,
}

impl Reliability {
    fn from_bits(v: u8) -> Self {
        match v & 7 {
            0 => Self::Unreliable,
            1 => Self::UnreliableSequenced,
            2 => Self::Reliable,
            3 => Self::ReliableOrdered,
            4 => Self::ReliableSequenced,
            5 => Self::UnreliableWithAck,
            6 => Self::ReliableWithAck,
            _ => Self::ReliableOrderedWithAck,
        }
    }

    pub fn is_reliable(self) -> bool {
        matches!(self, Self::Reliable | Self::ReliableOrdered | Self::ReliableSequenced | Self::ReliableWithAck | Self::ReliableOrderedWithAck)
    }

    pub fn is_sequenced(self) -> bool {
        matches!(self, Self::UnreliableSequenced | Self::ReliableSequenced)
    }

    pub fn is_ordered(self) -> bool {
        matches!(self, Self::ReliableOrdered | Self::ReliableOrderedWithAck)
    }

    fn has_order_info(self) -> bool {
        self.is_sequenced() || self.is_ordered()
    }

    /// Split messages must be reliable so every fragment eventually arrives.
    pub fn for_split(self) -> Self {
        match self {
            Self::Unreliable | Self::UnreliableWithAck => Self::Reliable,
            Self::UnreliableSequenced => Self::ReliableSequenced,
            other => other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Split {
    pub count: u32,
    pub id: u16,
    pub index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub reliability: Reliability,
    pub reliable_index: u32,
    pub sequence_index: u32,
    pub order_index: u32,
    pub order_channel: u8,
    pub split: Option<Split>,
    pub body: Bytes,
}

pub const MAX_FRAME_HEADER_LEN: usize = 1 + 2 + 3 + 3 + 4 + 10;

impl Frame {
    pub fn header_len(&self) -> usize {
        let r = self.reliability;
        3 + if r.is_reliable() { 3 } else { 0 }
            + if r.is_sequenced() { 3 } else { 0 }
            + if r.has_order_info() { 4 } else { 0 }
            + if self.split.is_some() { 10 } else { 0 }
    }

    pub fn encoded_len(&self) -> usize {
        self.header_len() + self.body.len()
    }

    pub fn encode(&self, out: &mut BytesMut) {
        let r = self.reliability;
        out.put_u8((r as u8) << 5 | if self.split.is_some() { FLAG_SPLIT } else { 0 });
        out.put_u16((self.body.len() * 8) as u16);
        if r.is_reliable() {
            out.put_u24_le(self.reliable_index);
        }
        if r.is_sequenced() {
            out.put_u24_le(self.sequence_index);
        }
        if r.has_order_info() {
            out.put_u24_le(self.order_index);
            out.put_u8(self.order_channel);
        }
        if let Some(s) = self.split {
            out.put_u32(s.count);
            out.put_u16(s.id);
            out.put_u32(s.index);
        }
        out.put_slice(&self.body);
    }

    /// `src` must be the buffer `r` reads from; the body is sliced from it without copying.
    pub fn decode(r: &mut Reader, src: &Bytes) -> Result<Self> {
        let flags = r.u8()?;
        let reliability = Reliability::from_bits(flags >> 5);
        let len = usize::from(r.u16_be()?).div_ceil(8);
        let reliable_index = if reliability.is_reliable() { r.u24_le()? } else { 0 };
        let sequence_index = if reliability.is_sequenced() { r.u24_le()? } else { 0 };
        let (order_index, order_channel) =
            if reliability.has_order_info() { (r.u24_le()?, r.u8()?) } else { (0, 0) };
        let split = if flags & FLAG_SPLIT != 0 {
            Some(Split { count: r.u32_be()?, id: r.u16_be()?, index: r.u32_be()? })
        } else {
            None
        };
        if len == 0 {
            return Err(WireError::Malformed("empty frame"));
        }
        let start = r.pos();
        r.take(len)?;
        Ok(Self { reliability, reliable_index, sequence_index, order_index, order_channel, split, body: src.slice(start..start + len) })
    }
}

pub fn put_datagram_header(out: &mut BytesMut, seq: u32) {
    out.put_u8(FLAG_VALID | FLAG_NEEDS_B_AND_AS);
    out.put_u24_le(seq);
}

/// Encodes sorted 24-bit sequence numbers as ACK/NACK range records, writing at most
/// `max_len` bytes. Returns how many sequence numbers were consumed.
pub fn encode_acks(out: &mut BytesMut, flag: u8, seqs: &[u32], max_len: usize) -> usize {
    out.put_u8(FLAG_VALID | flag);
    let count_pos = out.len();
    out.put_u16(0);
    let (mut records, mut used) = (0u16, 0);
    while used < seqs.len() {
        let start = seqs[used];
        let mut end_idx = used;
        while end_idx + 1 < seqs.len() && seqs[end_idx + 1] == (seqs[end_idx] + 1) & U24_MASK {
            end_idx += 1;
        }
        let single = end_idx == used;
        let record_len = if single { 4 } else { 7 };
        if out.len() + record_len > max_len || records == u16::MAX {
            break;
        }
        out.put_u8(u8::from(single));
        out.put_u24_le(start);
        if !single {
            out.put_u24_le(seqs[end_idx]);
        }
        records += 1;
        used = end_idx + 1;
    }
    out[count_pos..count_pos + 2].copy_from_slice(&records.to_be_bytes());
    used
}

/// Calls `f(start, end)` (inclusive, 24-bit) for each record of an ACK/NACK datagram.
pub fn decode_acks(data: &[u8], mut f: impl FnMut(u32, u32)) -> Result<()> {
    let mut r = Reader::new(data);
    r.u8()?;
    for _ in 0..r.u16_be()? {
        let single = r.bool()?;
        let start = r.u24_le()?;
        let end = if single { start } else { r.u24_le()? };
        f(start, end);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(reliability: Reliability, split: Option<Split>) -> Frame {
        Frame { reliability, reliable_index: 7, sequence_index: 3, order_index: 9, order_channel: 2, split, body: Bytes::from_static(b"hello") }
    }

    #[test]
    fn frame_roundtrip_all_reliabilities() {
        for bits in 0..8 {
            for split in [None, Some(Split { count: 3, id: 5, index: 1 })] {
                let mut f = frame(Reliability::from_bits(bits), split);
                if !f.reliability.is_reliable() { f.reliable_index = 0; }
                if !f.reliability.is_sequenced() { f.sequence_index = 0; }
                if !f.reliability.has_order_info() { f.order_index = 0; f.order_channel = 0; }
                let mut buf = BytesMut::new();
                f.encode(&mut buf);
                assert_eq!(buf.len(), f.encoded_len());
                let buf = buf.freeze();
                assert_eq!(Frame::decode(&mut Reader::new(&buf), &buf).unwrap(), f);
            }
        }
    }

    #[test]
    fn acks_merge_ranges_and_respect_limit() {
        let mut buf = BytesMut::new();
        let used = encode_acks(&mut buf, FLAG_ACK, &[1, 2, 3, 5, 0xff_ffff, 0], 1400);
        assert_eq!(used, 6);
        let mut got = vec![];
        decode_acks(&buf, |a, b| got.push((a, b))).unwrap();
        assert_eq!(got, vec![(1, 3), (5, 5), (0xff_ffff, 0)]);

        let mut buf = BytesMut::new();
        assert_eq!(encode_acks(&mut buf, FLAG_NACK, &[1, 3, 5], 3 + 8), 2);
    }
}
