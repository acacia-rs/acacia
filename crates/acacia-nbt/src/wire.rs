//! Wire primitives and the integer and length encodings that differ between flavours.

use bytes::{BufMut, BytesMut};

use crate::{Error, Result};

#[inline]
pub(crate) fn take<'a>(r: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    if r.len() < n {
        return Err(Error::Eof {
            needed: n,
            remaining: r.len(),
        });
    }
    let (head, tail) = r.split_at(n);
    *r = tail;
    Ok(head)
}

#[inline]
fn take_array<const N: usize>(r: &mut &[u8]) -> Result<[u8; N]> {
    Ok(take(r, N)?.try_into().expect("length checked"))
}

macro_rules! fixed {
    ($($read:ident $t:ty;)*) => {$(
        #[inline]
        pub(crate) fn $read(r: &mut &[u8]) -> Result<$t> {
            Ok(<$t>::from_le_bytes(take_array(r)?))
        }
    )*};
}

fixed! {
    read_u8 u8;
    read_i8 i8;
    read_lu16 u16;
    read_li16 i16;
    read_li32 i32;
    read_li64 i64;
    read_lf32 f32;
    read_lf64 f64;
}

macro_rules! varint {
    ($read:ident $t:ty, $max_bytes:expr) => {
        #[inline]
        fn $read(r: &mut &[u8]) -> Result<$t> {
            let mut value: $t = 0;
            for i in 0..$max_bytes {
                let b = read_u8(r)?;
                value |= ((b & 0x7f) as $t) << (7 * i);
                if b & 0x80 == 0 {
                    return Ok(value);
                }
            }
            Err(Error::VarIntTooLong)
        }
    };
}

/// Lengths and small ints are nearly all one byte, so only that case is inlined.
#[inline]
fn write_varint(w: &mut BytesMut, v: u64) {
    if v < 0x80 { put(w, [v as u8]) } else { write_long_varint(w, v) }
}

fn write_long_varint(w: &mut BytesMut, mut v: u64) {
    let (mut bytes, mut n) = ([0u8; 10], 0);
    while v >= 0x80 {
        bytes[n] = (v as u8) | 0x80;
        v >>= 7;
        n += 1;
    }
    bytes[n] = v as u8;
    put_slice(w, &bytes[..=n]);
}

/// Every write goes through here or [`put_slice`], not `BufMut::put_*` (README.md, "Performance").
#[inline]
pub fn put<const N: usize>(w: &mut BytesMut, bytes: [u8; N]) {
    put_slice(w, &bytes);
}

#[inline]
pub fn put_slice(w: &mut BytesMut, bytes: &[u8]) {
    if w.capacity() - w.len() < bytes.len() {
        w.reserve(bytes.len());
    }
    w.chunk_mut()[..bytes.len()].copy_from_slice(bytes);
    // SAFETY: the line above initialised that many bytes of spare capacity.
    unsafe { w.advance_mut(bytes.len()) }
}

varint!(read_varint32 u32, 5);
varint!(read_varint64 u64, 10);

pub trait Flavor {
    /// Longest string, in bytes, the length prefix can hold.
    const MAX_STR: usize;
    fn read_int(r: &mut &[u8]) -> Result<i32>;
    fn write_int(w: &mut BytesMut, v: i32);
    fn read_long(r: &mut &[u8]) -> Result<i64>;
    fn write_long(w: &mut BytesMut, v: i64);
    fn read_str_len(r: &mut &[u8]) -> Result<usize>;
    fn write_str_len(w: &mut BytesMut, n: usize);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Network;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LittleEndian;

impl Flavor for Network {
    const MAX_STR: usize = u32::MAX as usize;

    #[inline]
    fn read_int(r: &mut &[u8]) -> Result<i32> {
        let v = read_varint32(r)?;
        Ok(((v >> 1) as i32) ^ -((v & 1) as i32))
    }
    #[inline]
    fn write_int(w: &mut BytesMut, v: i32) {
        write_varint(w, u64::from(((v << 1) ^ (v >> 31)) as u32))
    }
    #[inline]
    fn read_long(r: &mut &[u8]) -> Result<i64> {
        let v = read_varint64(r)?;
        Ok(((v >> 1) as i64) ^ -((v & 1) as i64))
    }
    #[inline]
    fn write_long(w: &mut BytesMut, v: i64) {
        write_varint(w, ((v << 1) ^ (v >> 63)) as u64)
    }
    #[inline]
    fn read_str_len(r: &mut &[u8]) -> Result<usize> {
        Ok(read_varint32(r)? as usize)
    }
    #[inline]
    fn write_str_len(w: &mut BytesMut, n: usize) {
        write_varint(w, u64::from(n as u32))
    }
}

impl Flavor for LittleEndian {
    const MAX_STR: usize = u16::MAX as usize;

    #[inline]
    fn read_int(r: &mut &[u8]) -> Result<i32> {
        read_li32(r)
    }
    #[inline]
    fn write_int(w: &mut BytesMut, v: i32) {
        put(w, v.to_le_bytes())
    }
    #[inline]
    fn read_long(r: &mut &[u8]) -> Result<i64> {
        read_li64(r)
    }
    #[inline]
    fn write_long(w: &mut BytesMut, v: i64) {
        put(w, v.to_le_bytes())
    }
    #[inline]
    fn read_str_len(r: &mut &[u8]) -> Result<usize> {
        Ok(read_lu16(r)? as usize)
    }
    #[inline]
    fn write_str_len(w: &mut BytesMut, n: usize) {
        put(w, (n as u16).to_le_bytes())
    }
}

/// An array or list length: an int, rejected when negative.
#[inline]
pub(crate) fn read_len<F: Flavor>(r: &mut &[u8]) -> Result<usize> {
    let n = F::read_int(r)?;
    usize::try_from(n).map_err(|_| Error::InvalidLength(n as i64))
}

/// A list's element tag and length.
#[inline]
pub(crate) fn read_list_header<F: Flavor>(r: &mut &[u8]) -> Result<(u8, usize)> {
    let tag = read_u8(r)?;
    let n = read_len::<F>(r)?;
    // End elements take no bytes, so the count would not be bounded by the input.
    if tag == 0 && n != 0 {
        return Err(Error::EndList(n));
    }
    Ok((tag, n))
}
