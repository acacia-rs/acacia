//! Network NBT as a value tree (big-endian, nameless root), for payloads the bot reads or builds: dialogs and
//! their custom click payloads. Other NBT is skipped with [`crate::nbt`].

use crate::nbt::{TAG_COMPOUND, TAG_END, TAG_STRING};
use crate::wire::take;
use crate::{Error, Wire};

const MAX_DEPTH: u32 = 512;

#[derive(Debug, Clone, PartialEq)]
pub enum Tag {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(Vec<Tag>),
    Compound(Vec<(String, Tag)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl Tag {
    pub fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        if let Tag::String(s) = self { Some(s) } else { None }
    }

    /// Any number as `f64`; a byte reads as a boolean's 0 or 1.
    pub fn as_f64(&self) -> Option<f64> {
        Some(match *self {
            Tag::Byte(v) => f64::from(v),
            Tag::Short(v) => f64::from(v),
            Tag::Int(v) => f64::from(v),
            Tag::Long(v) => v as f64,
            Tag::Float(v) => f64::from(v),
            Tag::Double(v) => v,
            _ => return None,
        })
    }

    pub fn as_bool(&self) -> Option<bool> {
        self.as_f64().map(|v| v != 0.0)
    }

    fn kind(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Short(_) => 2,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::Double(_) => 6,
            Tag::ByteArray(_) => 7,
            Tag::String(_) => TAG_STRING,
            Tag::List(_) => 9,
            Tag::Compound(_) => TAG_COMPOUND,
            Tag::IntArray(_) => 11,
            Tag::LongArray(_) => 12,
        }
    }

    /// A root tag: its type byte, then the payload; `TAG_End` reads as `None`.
    pub fn read_root(input: &mut &[u8]) -> Result<Option<Tag>, Error> {
        match u8::read(input)? {
            TAG_END => Ok(None),
            kind => read_payload(kind, input, 0).map(Some),
        }
    }

    pub fn write_root(&self, out: &mut Vec<u8>) {
        out.push(self.kind());
        self.write_payload(out);
    }

    fn write_payload(&self, out: &mut Vec<u8>) {
        match self {
            Tag::Byte(v) => v.write(out),
            Tag::Short(v) => v.write(out),
            Tag::Int(v) => v.write(out),
            Tag::Long(v) => v.write(out),
            Tag::Float(v) => v.write(out),
            Tag::Double(v) => v.write(out),
            Tag::ByteArray(v) => {
                (v.len() as i32).write(out);
                v.iter().for_each(|b| b.write(out));
            }
            Tag::String(s) => write_string(s, out),
            Tag::List(items) => {
                out.push(items.first().map_or(TAG_END, Tag::kind));
                (items.len() as i32).write(out);
                items.iter().for_each(|t| t.write_payload(out));
            }
            Tag::Compound(entries) => {
                for (key, value) in entries {
                    out.push(value.kind());
                    write_string(key, out);
                    value.write_payload(out);
                }
                out.push(TAG_END);
            }
            Tag::IntArray(v) => {
                (v.len() as i32).write(out);
                v.iter().for_each(|n| n.write(out));
            }
            Tag::LongArray(v) => {
                (v.len() as i32).write(out);
                v.iter().for_each(|n| n.write(out));
            }
        }
    }
}

// Modified UTF-8 matches UTF-8 except for NUL and supplementary characters, which the bot's payloads avoid.
fn write_string(s: &str, out: &mut Vec<u8>) {
    (s.len().min(usize::from(u16::MAX)) as u16).write(out);
    out.extend_from_slice(&s.as_bytes()[..s.len().min(usize::from(u16::MAX))]);
}

fn read_string(input: &mut &[u8]) -> Result<String, Error> {
    let len = usize::from(u16::read(input)?);
    Ok(String::from_utf8_lossy(take(input, len)?).into_owned())
}

fn read_len(input: &mut &[u8]) -> Result<usize, Error> {
    let len = i32::read(input)?;
    usize::try_from(len).map_err(|_| Error::NegativeLength(len))
}

fn read_payload(kind: u8, input: &mut &[u8], depth: u32) -> Result<Tag, Error> {
    if depth >= MAX_DEPTH {
        return Err(Error::TooDeep(MAX_DEPTH));
    }
    let array = |input: &mut &[u8], width: usize| -> Result<usize, Error> {
        let n = read_len(input)?;
        // Every element needs `width` bytes: refuse a length the input cannot hold before allocating.
        if n.saturating_mul(width) > input.len() { Err(Error::Invalid("NBT array length")) } else { Ok(n) }
    };
    Ok(match kind {
        1 => Tag::Byte(i8::read(input)?),
        2 => Tag::Short(i16::read(input)?),
        3 => Tag::Int(i32::read(input)?),
        4 => Tag::Long(i64::read(input)?),
        5 => Tag::Float(f32::read(input)?),
        6 => Tag::Double(f64::read(input)?),
        7 => Tag::ByteArray((0..array(input, 1)?).map(|_| i8::read(input)).collect::<Result<_, _>>()?),
        TAG_STRING => Tag::String(read_string(input)?),
        9 => {
            let element = u8::read(input)?;
            let count = read_len(input)?;
            if element == TAG_END && count > 0 {
                return Err(Error::Invalid("NBT list element type"));
            }
            if count > input.len() {
                return Err(Error::Invalid("NBT list length"));
            }
            Tag::List((0..count).map(|_| read_payload(element, input, depth + 1)).collect::<Result<_, _>>()?)
        }
        TAG_COMPOUND => {
            let mut entries = Vec::new();
            loop {
                let entry = u8::read(input)?;
                if entry == TAG_END {
                    break Tag::Compound(entries);
                }
                let key = read_string(input)?;
                entries.push((key, read_payload(entry, input, depth + 1)?));
            }
        }
        11 => Tag::IntArray((0..array(input, 4)?).map(|_| i32::read(input)).collect::<Result<_, _>>()?),
        12 => Tag::LongArray((0..array(input, 8)?).map(|_| i64::read(input)).collect::<Result<_, _>>()?),
        _ => return Err(Error::Invalid("NBT tag type")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_type_round_trips() {
        let tag = Tag::Compound(vec![
            ("b".into(), Tag::Byte(-1)),
            ("s".into(), Tag::Short(2)),
            ("i".into(), Tag::Int(3)),
            ("l".into(), Tag::Long(4)),
            ("f".into(), Tag::Float(0.5)),
            ("d".into(), Tag::Double(0.25)),
            ("ba".into(), Tag::ByteArray(vec![1, 2])),
            ("str".into(), Tag::String("hé".into())),
            ("list".into(), Tag::List(vec![Tag::Int(1), Tag::Int(2)])),
            ("empty".into(), Tag::List(Vec::new())),
            ("c".into(), Tag::Compound(vec![("x".into(), Tag::String("y".into()))])),
            ("ia".into(), Tag::IntArray(vec![7])),
            ("la".into(), Tag::LongArray(vec![8])),
        ]);
        let mut out = Vec::new();
        tag.write_root(&mut out);
        let mut input = &out[..];
        assert_eq!(Tag::read_root(&mut input), Ok(Some(tag)));
        assert!(input.is_empty());
    }

    #[test]
    fn hostile_lengths_fail_before_allocating() {
        assert_eq!(Tag::read_root(&mut &[11, 0x7f, 0xff, 0xff, 0xff][..]), Err(Error::Invalid("NBT array length")));
        assert_eq!(Tag::read_root(&mut &[9, 1, 0x7f, 0xff, 0xff, 0xff][..]), Err(Error::Invalid("NBT list length")));
        assert_eq!(Tag::read_root(&mut &[0][..]), Ok(None));
    }
}
