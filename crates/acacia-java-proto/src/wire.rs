#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("packet ended early")]
    Eof,
    #[error("varint longer than 5 bytes")]
    VarIntTooLong,
    #[error("negative length {0}")]
    NegativeLength(i32),
    #[error("string is not UTF-8")]
    BadUtf8,
}

/// A value's layout on the wire.
pub trait Wire: Sized {
    fn read(input: &mut &[u8]) -> Result<Self, Error>;
    fn write(&self, out: &mut Vec<u8>);
}

pub fn read_varint(input: &mut &[u8]) -> Result<i32, Error> {
    let mut value = 0u32;
    for i in 0..5 {
        let (&byte, rest) = input.split_first().ok_or(Error::Eof)?;
        *input = rest;
        value |= u32::from(byte & 0x7f) << (7 * i);
        if byte & 0x80 == 0 {
            return Ok(value as i32);
        }
    }
    Err(Error::VarIntTooLong)
}

pub fn write_varint(value: i32, out: &mut Vec<u8>) {
    let mut v = value as u32;
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

fn take<'a>(input: &mut &'a [u8], n: usize) -> Result<&'a [u8], Error> {
    let (head, rest) = input.split_at_checked(n).ok_or(Error::Eof)?;
    *input = rest;
    Ok(head)
}

fn read_len(input: &mut &[u8]) -> Result<usize, Error> {
    let len = read_varint(input)?;
    usize::try_from(len).map_err(|_| Error::NegativeLength(len))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VarInt(pub i32);

impl Wire for VarInt {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        read_varint(input).map(VarInt)
    }
    fn write(&self, out: &mut Vec<u8>) {
        write_varint(self.0, out);
    }
}

macro_rules! big_endian {
    ($($t:ty),*) => {$(
        impl Wire for $t {
            fn read(input: &mut &[u8]) -> Result<Self, Error> {
                Ok(<$t>::from_be_bytes(take(input, size_of::<$t>())?.try_into().expect("exact length")))
            }
            fn write(&self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_be_bytes());
            }
        }
    )*};
}
// u128: a UUID is two big-endian longs.
big_endian!(u8, i8, u16, i16, i32, i64, u128, f32, f64);

impl Wire for bool {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(u8::read(input)? != 0)
    }
    fn write(&self, out: &mut Vec<u8>) {
        out.push(u8::from(*self));
    }
}

impl Wire for String {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let len = read_len(input)?;
        String::from_utf8(take(input, len)?.to_vec()).map_err(|_| Error::BadUtf8)
    }
    fn write(&self, out: &mut Vec<u8>) {
        write_varint(self.len() as i32, out);
        out.extend_from_slice(self.as_bytes());
    }
}

impl<T: Wire> Wire for Vec<T> {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let len = read_len(input)?;
        // Capped: the length is untrusted, and each element takes at least a byte unless the type is empty.
        let mut items = Vec::with_capacity(len.min(input.len()));
        for _ in 0..len {
            items.push(T::read(input)?);
        }
        Ok(items)
    }
    fn write(&self, out: &mut Vec<u8>) {
        write_varint(self.len() as i32, out);
        self.iter().for_each(|item| item.write(out));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_match_the_protocol_examples() {
        for (value, bytes) in [(0, &[0x00][..]), (127, &[0x7f]), (128, &[0x80, 0x01]), (25565, &[0xdd, 0xc7, 0x01]), (-1, &[0xff, 0xff, 0xff, 0xff, 0x0f])] {
            let mut out = Vec::new();
            write_varint(value, &mut out);
            assert_eq!(out, bytes);
            assert_eq!(read_varint(&mut &out[..]), Ok(value));
        }
        assert_eq!(read_varint(&mut &[0x80, 0x80, 0x80, 0x80, 0x80, 0x01][..]), Err(Error::VarIntTooLong));
    }

    #[test]
    fn strings_and_lists_round_trip_and_reject_short_input() {
        let list = vec!["a".to_string(), "bc".to_string()];
        let mut out = Vec::new();
        list.write(&mut out);
        assert_eq!(<Vec<String>>::read(&mut &out[..]), Ok(list));
        assert_eq!(String::read(&mut &[5, b'a'][..]), Err(Error::Eof));
    }
}
