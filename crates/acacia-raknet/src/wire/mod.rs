pub mod connected;
pub mod datagram;
pub mod offline;

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use bytes::BufMut;

pub const MAGIC: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];

pub const U24_MASK: u32 = 0xff_ffff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    Truncated,
    Malformed(&'static str),
}

impl std::fmt::Display for WireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireError::Truncated => f.write_str("truncated packet"),
            WireError::Malformed(what) => write!(f, "malformed packet: {what}"),
        }
    }
}

impl std::error::Error for WireError {}

pub type Result<T> = std::result::Result<T, WireError>;

pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or(WireError::Truncated)?;
        let out = self.buf.get(self.pos..end).ok_or(WireError::Truncated)?;
        self.pos = end;
        Ok(out)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().expect("take returned N bytes"))
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn bool(&mut self) -> Result<bool> {
        Ok(self.u8()? != 0)
    }

    pub fn u16_be(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub fn u24_le(&mut self) -> Result<u32> {
        let b = self.array::<3>()?;
        Ok(u32::from(b[0]) | u32::from(b[1]) << 8 | u32::from(b[2]) << 16)
    }

    pub fn u32_be(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub fn u64_be(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    pub fn i64_be(&mut self) -> Result<i64> {
        Ok(i64::from_be_bytes(self.array()?))
    }

    pub fn magic(&mut self) -> Result<()> {
        if self.array::<16>()? == MAGIC { Ok(()) } else { Err(WireError::Malformed("magic")) }
    }

    pub fn addr(&mut self) -> Result<SocketAddr> {
        match self.u8()? {
            4 => {
                let b = self.array::<4>()?;
                let ip = Ipv4Addr::new(!b[0], !b[1], !b[2], !b[3]);
                Ok(SocketAddr::new(ip.into(), self.u16_be()?))
            }
            6 => {
                self.take(2)?; // address family, little endian
                let port = self.u16_be()?;
                self.take(4)?; // flow info
                let ip = Ipv6Addr::from(self.array::<16>()?);
                self.take(4)?; // scope id
                Ok(SocketAddr::new(ip.into(), port))
            }
            _ => Err(WireError::Malformed("address version")),
        }
    }
}

pub trait WriteExt: BufMut {
    fn put_u24_le(&mut self, v: u32) {
        self.put_uint_le(u64::from(v & U24_MASK), 3);
    }

    fn put_magic(&mut self) {
        self.put_slice(&MAGIC);
    }

    fn put_addr(&mut self, addr: SocketAddr) {
        match addr {
            SocketAddr::V4(a) => {
                self.put_u8(4);
                for b in a.ip().octets() {
                    self.put_u8(!b);
                }
                self.put_u16(a.port());
            }
            SocketAddr::V6(a) => {
                self.put_u8(6);
                self.put_u16_le(23); // AF_INET6 as the Windows-built Bedrock server expects
                self.put_u16(a.port());
                self.put_u32(a.flowinfo());
                self.put_slice(&a.ip().octets());
                self.put_u32(a.scope_id());
            }
        }
    }
}

impl<T: BufMut + ?Sized> WriteExt for T {}

/// Maps a wrapping 24-bit counter onto the 64-bit counter closest to `reference`.
pub fn unwrap24(reference: u64, idx: u32) -> u64 {
    let delta = idx.wrapping_sub(reference as u32) & U24_MASK;
    if delta < 0x80_0000 {
        reference + u64::from(delta)
    } else {
        reference.saturating_sub(u64::from(0x100_0000 - delta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;

    #[test]
    fn addr_roundtrip() {
        for a in ["1.2.3.4:19132", "[2001:db8::1]:19133"] {
            let addr: SocketAddr = a.parse().unwrap();
            let mut buf = BytesMut::new();
            buf.put_addr(addr);
            assert_eq!(Reader::new(&buf).addr().unwrap(), addr);
        }
    }

    #[test]
    fn unwrap24_handles_wraparound() {
        assert_eq!(unwrap24(5, 7), 7);
        assert_eq!(unwrap24(5, 3), 3);
        assert_eq!(unwrap24(0xff_fffe, 1), 0x100_0001);
        assert_eq!(unwrap24(0x100_0001, 0xff_fffe), 0xff_fffe);
        assert_eq!(unwrap24(0, 0xff_ffff), 0);
    }
}
