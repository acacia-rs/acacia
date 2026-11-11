//! Skipping NBT in its network form (`FriendlyByteBuf.readNbt`): a type byte, then a nameless payload.

use crate::wire::take;
use crate::{Error, Wire};

/// `NbtAccounter`'s depth limit.
const MAX_DEPTH: u32 = 512;

pub const TAG_END: u8 = 0;
pub const TAG_STRING: u8 = 8;
pub const TAG_COMPOUND: u8 = 10;

/// Skips a root tag; `Ok(None)` for `TAG_End`, which `readNbt` reads as no tag at all. Returns the root's type.
pub fn skip_root(input: &mut &[u8]) -> Result<Option<u8>, Error> {
    match u8::read(input)? {
        TAG_END => Ok(None),
        kind => skip_payload(kind, input, 0).map(|()| Some(kind)),
    }
}

/// A root tag that must be present, as `ByteBufCodecs.TAG` (and every `fromCodec` stream codec) demands.
pub fn skip_tag(input: &mut &[u8]) -> Result<u8, Error> {
    skip_root(input)?.ok_or(Error::Invalid("NBT root tag"))
}

/// `ByteBufCodecs.COMPOUND_TAG`.
pub fn skip_compound(input: &mut &[u8]) -> Result<(), Error> {
    match skip_tag(input)? {
        TAG_COMPOUND => Ok(()),
        _ => Err(Error::Invalid("NBT compound root")),
    }
}

/// A root `TAG_String`'s value, as a plain text component is sent; `None` for any other root.
pub fn read_root_string(mut input: &[u8]) -> Option<String> {
    let (&TAG_STRING, rest) = input.split_first()? else { return None };
    input = rest;
    let len = u16::read(&mut input).ok()?;
    // Modified UTF-8 matches UTF-8 except for NUL and supplementary characters.
    String::from_utf8(take(&mut input, usize::from(len)).ok()?.to_vec()).ok()
}

fn array_len(input: &mut &[u8], width: usize) -> Result<usize, Error> {
    let len = i32::read(input)?;
    usize::try_from(len).map(|n| n * width).map_err(|_| Error::NegativeLength(len))
}

fn skip_payload(kind: u8, input: &mut &[u8], depth: u32) -> Result<(), Error> {
    if depth >= MAX_DEPTH {
        return Err(Error::TooDeep(MAX_DEPTH));
    }
    let n = match kind {
        1 => 1,
        2 => 2,
        3 | 5 => 4,
        4 | 6 => 8,
        7 => array_len(input, 1)?,
        TAG_STRING => usize::from(u16::read(input)?),
        9 => {
            let element = u8::read(input)?;
            let count = i32::read(input)?;
            if element == TAG_END && count > 0 {
                return Err(Error::Invalid("NBT list element type"));
            }
            for _ in 0..count {
                skip_payload(element, input, depth + 1)?;
            }
            0
        }
        TAG_COMPOUND => {
            loop {
                let entry = u8::read(input)?;
                if entry == TAG_END {
                    break;
                }
                let name = usize::from(u16::read(input)?);
                take(input, name)?;
                skip_payload(entry, input, depth + 1)?;
            }
            0
        }
        11 => array_len(input, 4)?,
        12 => array_len(input, 8)?,
        _ => return Err(Error::Invalid("NBT tag type")),
    };
    take(input, n).map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_every_tag_type_in_a_compound() {
        #[rustfmt::skip]
        let tag = [
            10,
            1, 0, 1, b'b', 7,
            2, 0, 0, 0, 1,
            3, 0, 0, 0, 0, 0, 1,
            4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
            5, 0, 0, 0, 0, 0, 0,
            6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            7, 0, 0, 0, 0, 0, 2, 9, 9,
            8, 0, 1, b's', 0, 2, b'h', b'i',
            9, 0, 0, 3, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 2,
            9, 0, 0, 0, 0, 0, 0, 0,
            10, 0, 1, b'c', 0,
            11, 0, 0, 0, 0, 0, 1, 0, 0, 0, 5,
            12, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 5,
            0,
            0xaa,
        ];
        let mut input = &tag[..];
        assert_eq!(skip_tag(&mut input), Ok(TAG_COMPOUND));
        assert_eq!(input, [0xaa]);
    }

    #[test]
    fn end_root_is_absent_and_bad_input_fails() {
        assert_eq!(skip_root(&mut &[0][..]), Ok(None));
        assert_eq!(skip_tag(&mut &[0][..]), Err(Error::Invalid("NBT root tag")));
        assert_eq!(skip_tag(&mut &[13][..]), Err(Error::Invalid("NBT tag type")));
        assert_eq!(skip_tag(&mut &[9, 0, 0, 0, 0, 1][..]), Err(Error::Invalid("NBT list element type")));
        assert_eq!(skip_tag(&mut &[11, 0xff, 0xff, 0xff, 0xff][..]), Err(Error::NegativeLength(-1)));
        assert_eq!(skip_compound(&mut &[8, 0, 0][..]), Err(Error::Invalid("NBT compound root")));
    }

    #[test]
    fn deep_lists_stop_at_the_accounter_limit() {
        let mut tag = vec![9];
        for _ in 0..600 {
            tag.extend([9, 0, 0, 0, 1]);
        }
        assert_eq!(skip_tag(&mut &tag[..]), Err(Error::TooDeep(MAX_DEPTH)));
    }

    #[test]
    fn reads_a_plain_string_root() {
        assert_eq!(read_root_string(&[8, 0, 2, b'h', b'i']), Some("hi".into()));
        assert_eq!(read_root_string(&[10, 0]), None);
    }
}
