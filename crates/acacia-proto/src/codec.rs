//! Wire primitives shared by generated code and hand-written types.

use acacia_nbt::{put, put_slice};
use bytes::{Bytes, BytesMut};

use crate::DecodeError;
use crate::strict::{Leniency, note};

pub type Result<T> = std::result::Result<T, DecodeError>;

#[inline]
pub fn take<'a>(r: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
    if r.len() < n {
        return Err(DecodeError::Eof {
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
    ($($read:ident $write:ident $t:ty, $from:ident $to:ident;)*) => {$(
        #[inline]
        pub fn $read(r: &mut &[u8]) -> Result<$t> {
            Ok(<$t>::$from(take_array(r)?))
        }
        #[inline]
        pub fn $write(w: &mut BytesMut, v: $t) {
            put(w, v.$to());
        }
    )*};
}

fixed! {
    read_u8 write_u8 u8, from_le_bytes to_le_bytes;
    read_i8 write_i8 i8, from_le_bytes to_le_bytes;
    read_lu16 write_lu16 u16, from_le_bytes to_le_bytes;
    read_li16 write_li16 i16, from_le_bytes to_le_bytes;
    read_lu32 write_lu32 u32, from_le_bytes to_le_bytes;
    read_li32 write_li32 i32, from_le_bytes to_le_bytes;
    read_lu64 write_lu64 u64, from_le_bytes to_le_bytes;
    read_li64 write_li64 i64, from_le_bytes to_le_bytes;
    read_lf32 write_lf32 f32, from_le_bytes to_le_bytes;
    read_lf64 write_lf64 f64, from_le_bytes to_le_bytes;
    read_u16 write_u16 u16, from_be_bytes to_be_bytes;
    read_i16 write_i16 i16, from_be_bytes to_be_bytes;
    read_u32 write_u32 u32, from_be_bytes to_be_bytes;
    read_i32 write_i32 i32, from_be_bytes to_be_bytes;
    read_u64 write_u64 u64, from_be_bytes to_be_bytes;
    read_i64 write_i64 i64, from_be_bytes to_be_bytes;
    read_f32 write_f32 f32, from_be_bytes to_be_bytes;
    read_f64 write_f64 f64, from_be_bytes to_be_bytes;
}

#[inline]
pub fn read_bool(r: &mut &[u8]) -> Result<bool> {
    let b = read_u8(r)?;
    if b > 1 {
        note(Leniency::Bool(b));
    }
    Ok(b != 0)
}

#[inline]
pub fn write_bool(w: &mut BytesMut, v: bool) {
    put(w, [v as u8]);
}

macro_rules! varint {
    ($read:ident $write:ident $t:ty, $max_bytes:expr) => {
        #[inline]
        pub fn $read(r: &mut &[u8]) -> Result<$t> {
            let mut value: $t = 0;
            for i in 0..$max_bytes {
                let b = read_u8(r)?;
                value |= ((b & 0x7f) as $t) << (7 * i);
                if b & 0x80 == 0 {
                    return Ok(value);
                }
            }
            Err(DecodeError::VarIntTooLong)
        }
        #[inline]
        pub fn $write(w: &mut BytesMut, v: $t) {
            // Only the one-byte case is inlined (acacia-nbt README.md, "Performance").
            fn long(w: &mut BytesMut, mut v: $t) {
                let (mut bytes, mut n) = ([0u8; $max_bytes], 0);
                while v >= 0x80 {
                    bytes[n] = (v as u8) | 0x80;
                    v >>= 7;
                    n += 1;
                }
                bytes[n] = v as u8;
                put_slice(w, &bytes[..=n]);
            }
            if v < 0x80 { put(w, [v as u8]) } else { long(w, v) }
        }
    };
}

varint!(read_varint write_varint u32, 5);
varint!(read_varint64 write_varint64 u64, 10);
varint!(read_varint128 write_varint128 u128, 19);

#[inline]
pub fn read_zigzag32(r: &mut &[u8]) -> Result<i32> {
    let v = read_varint(r)?;
    Ok(((v >> 1) as i32) ^ -((v & 1) as i32))
}

#[inline]
pub fn write_zigzag32(w: &mut BytesMut, v: i32) {
    write_varint(w, ((v << 1) ^ (v >> 31)) as u32);
}

#[inline]
pub fn read_zigzag64(r: &mut &[u8]) -> Result<i64> {
    let v = read_varint64(r)?;
    Ok(((v >> 1) as i64) ^ -((v & 1) as i64))
}

#[inline]
pub fn write_zigzag64(w: &mut BytesMut, v: i64) {
    write_varint64(w, ((v << 1) ^ (v >> 63)) as u64);
}

/// Converts a decoded count/length to `usize`, rejecting negatives.
#[inline]
pub fn to_len<T: TryInto<usize> + Into<i128> + Copy>(n: T) -> Result<usize> {
    n.try_into()
        .map_err(|_| DecodeError::InvalidLength(n.into() as i64))
}

/// Initial `Vec` capacity for a decoded count; bounded by the remaining input so a hostile count can't OOM.
#[inline]
pub fn cap(n: usize, r: &[u8]) -> usize {
    n.min(r.len())
}

/// Decodes UTF-8, falling back to lossy replacement rather than failing the whole packet.
pub fn read_utf8(r: &mut &[u8], n: usize) -> Result<String> {
    let b = take(r, n)?;
    Ok(match std::str::from_utf8(b) {
        Ok(s) => s.to_owned(),
        Err(_) => {
            note(Leniency::Utf8);
            String::from_utf8_lossy(b).into_owned()
        }
    })
}

pub fn read_latin1(r: &mut &[u8], n: usize) -> Result<String> {
    Ok(take(r, n)?.iter().map(|&b| b as char).collect())
}

pub fn latin1_bytes(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| if (c as u32) < 256 { c as u8 } else { b'?' })
        .collect()
}

pub fn read_bytes(r: &mut &[u8], n: usize) -> Result<Bytes> {
    Ok(Bytes::copy_from_slice(take(r, n)?))
}

pub fn read_rest(r: &mut &[u8]) -> Bytes {
    let b = Bytes::copy_from_slice(r);
    *r = &[];
    b
}

pub fn read_uuid(r: &mut &[u8]) -> Result<crate::manual::Uuid> {
    Ok(crate::manual::Uuid(take_array(r)?))
}

pub fn write_uuid(w: &mut BytesMut, v: &crate::manual::Uuid) {
    put(w, v.0);
}

#[inline]
pub fn write_slice(w: &mut BytesMut, b: &[u8]) {
    put_slice(w, b);
}

/// Writes a length-prefixed sub-buffer: `body` is encoded to scratch space first, then `prefix(len)` and the bytes.
pub fn write_encapsulated(
    w: &mut BytesMut,
    body: impl FnOnce(&mut BytesMut),
    prefix: impl FnOnce(&mut BytesMut, usize),
) {
    let mut tmp = BytesMut::new();
    body(&mut tmp);
    prefix(w, tmp.len());
    put_slice(w, &tmp);
}
