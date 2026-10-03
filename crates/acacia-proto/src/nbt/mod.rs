//! Bedrock NBT in its two wire flavours:
//! - [`Network`]: little-endian with varint lengths and zigzag ints (minecraft-data `nbt`).
//! - [`LittleEndian`]: plain little-endian (minecraft-data `lnbt`, used inside item extra data).

use bytes::{BufMut, BytesMut};

use crate::DecodeError;
use crate::codec::*;

mod skip;
pub use skip::{read_loop, skip, write_loop};

const MAX_DEPTH: usize = 512;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(List),
    /// Entries keep wire order so re-encoding is byte-exact.
    Compound(Vec<(String, Value)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

/// A list keeps its element tag so empty lists re-encode byte-exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct List {
    pub tag: u8,
    pub items: Vec<Value>,
}

/// A root (named) tag. An `End` root is the one-byte "no NBT" marker.
#[derive(Debug, Clone, PartialEq)]
pub struct Nbt {
    pub name: String,
    pub value: Value,
}

impl Default for Nbt {
    fn default() -> Self {
        Nbt {
            name: String::new(),
            value: Value::End,
        }
    }
}

impl Value {
    pub fn tag(&self) -> u8 {
        match self {
            Value::End => 0,
            Value::Byte(_) => 1,
            Value::Short(_) => 2,
            Value::Int(_) => 3,
            Value::Long(_) => 4,
            Value::Float(_) => 5,
            Value::Double(_) => 6,
            Value::ByteArray(_) => 7,
            Value::String(_) => 8,
            Value::List(_) => 9,
            Value::Compound(_) => 10,
            Value::IntArray(_) => 11,
            Value::LongArray(_) => 12,
        }
    }

    /// Looks up a key in a compound.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Compound(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

/// Integer and length encodings that differ between flavours.
pub trait Flavor {
    fn read_int(r: &mut &[u8]) -> Result<i32>;
    fn write_int(w: &mut BytesMut, v: i32);
    fn read_long(r: &mut &[u8]) -> Result<i64>;
    fn write_long(w: &mut BytesMut, v: i64);
    fn read_str_len(r: &mut &[u8]) -> Result<usize>;
    fn write_str_len(w: &mut BytesMut, n: usize);
}

pub struct Network;
pub struct LittleEndian;

impl Flavor for Network {
    fn read_int(r: &mut &[u8]) -> Result<i32> {
        read_zigzag32(r)
    }
    fn write_int(w: &mut BytesMut, v: i32) {
        write_zigzag32(w, v)
    }
    fn read_long(r: &mut &[u8]) -> Result<i64> {
        read_zigzag64(r)
    }
    fn write_long(w: &mut BytesMut, v: i64) {
        write_zigzag64(w, v)
    }
    fn read_str_len(r: &mut &[u8]) -> Result<usize> {
        Ok(read_varint(r)? as usize)
    }
    fn write_str_len(w: &mut BytesMut, n: usize) {
        write_varint(w, n as u32)
    }
}

impl Flavor for LittleEndian {
    fn read_int(r: &mut &[u8]) -> Result<i32> {
        read_li32(r)
    }
    fn write_int(w: &mut BytesMut, v: i32) {
        write_li32(w, v)
    }
    fn read_long(r: &mut &[u8]) -> Result<i64> {
        read_li64(r)
    }
    fn write_long(w: &mut BytesMut, v: i64) {
        write_li64(w, v)
    }
    fn read_str_len(r: &mut &[u8]) -> Result<usize> {
        Ok(read_lu16(r)? as usize)
    }
    fn write_str_len(w: &mut BytesMut, n: usize) {
        write_lu16(w, n as u16)
    }
}

fn read_len<F: Flavor>(r: &mut &[u8]) -> Result<usize> {
    to_len(F::read_int(r)?)
}

fn read_str<F: Flavor>(r: &mut &[u8]) -> Result<String> {
    let n = F::read_str_len(r)?;
    read_utf8(r, n)
}

fn write_str<F: Flavor>(w: &mut BytesMut, s: &str) {
    F::write_str_len(w, s.len());
    w.put_slice(s.as_bytes());
}

/// Reads a root tag: type byte, then (unless `End`) name and payload.
pub fn read<F: Flavor>(r: &mut &[u8]) -> Result<Nbt> {
    let tag = read_u8(r)?;
    if tag == 0 {
        return Ok(Nbt::default());
    }
    let name = read_str::<F>(r)?;
    let value = read_payload::<F>(r, tag, 0)?;
    Ok(Nbt { name, value })
}

pub fn write<F: Flavor>(w: &mut BytesMut, nbt: &Nbt) {
    let tag = nbt.value.tag();
    w.put_u8(tag);
    if tag != 0 {
        write_str::<F>(w, &nbt.name);
        write_payload::<F>(w, &nbt.value);
    }
}

fn read_payload<F: Flavor>(r: &mut &[u8], tag: u8, depth: usize) -> Result<Value> {
    if depth > MAX_DEPTH {
        return Err(DecodeError::NbtTooDeep(MAX_DEPTH));
    }
    Ok(match tag {
        0 => Value::End,
        1 => Value::Byte(read_i8(r)?),
        2 => Value::Short(read_li16(r)?),
        3 => Value::Int(F::read_int(r)?),
        4 => Value::Long(F::read_long(r)?),
        5 => Value::Float(read_lf32(r)?),
        6 => Value::Double(read_lf64(r)?),
        7 => {
            let n = read_len::<F>(r)?;
            Value::ByteArray(take(r, n)?.to_vec())
        }
        8 => Value::String(read_str::<F>(r)?),
        9 => {
            let tag = read_u8(r)?;
            let n = read_len::<F>(r)?;
            let mut items = Vec::with_capacity(cap(n, r));
            for _ in 0..n {
                items.push(read_payload::<F>(r, tag, depth + 1)?);
            }
            Value::List(List { tag, items })
        }
        10 => {
            let mut entries = Vec::new();
            loop {
                let tag = read_u8(r)?;
                if tag == 0 {
                    break;
                }
                let name = read_str::<F>(r)?;
                entries.push((name, read_payload::<F>(r, tag, depth + 1)?));
            }
            Value::Compound(entries)
        }
        11 => {
            let n = read_len::<F>(r)?;
            let mut v = Vec::with_capacity(cap(n, r));
            for _ in 0..n {
                v.push(F::read_int(r)?);
            }
            Value::IntArray(v)
        }
        12 => {
            let n = read_len::<F>(r)?;
            let mut v = Vec::with_capacity(cap(n, r));
            for _ in 0..n {
                v.push(F::read_long(r)?);
            }
            Value::LongArray(v)
        }
        _ => return Err(DecodeError::Nbt("unknown tag type")),
    })
}

fn write_payload<F: Flavor>(w: &mut BytesMut, v: &Value) {
    match v {
        Value::End => {}
        Value::Byte(x) => write_i8(w, *x),
        Value::Short(x) => write_li16(w, *x),
        Value::Int(x) => F::write_int(w, *x),
        Value::Long(x) => F::write_long(w, *x),
        Value::Float(x) => write_lf32(w, *x),
        Value::Double(x) => write_lf64(w, *x),
        Value::ByteArray(b) => {
            F::write_int(w, b.len() as i32);
            w.put_slice(b);
        }
        Value::String(s) => write_str::<F>(w, s),
        Value::List(l) => {
            w.put_u8(l.tag);
            F::write_int(w, l.items.len() as i32);
            for item in &l.items {
                write_payload::<F>(w, item);
            }
        }
        Value::Compound(entries) => {
            for (name, value) in entries {
                w.put_u8(value.tag());
                write_str::<F>(w, name);
                write_payload::<F>(w, value);
            }
            w.put_u8(0);
        }
        Value::IntArray(a) => {
            F::write_int(w, a.len() as i32);
            a.iter().for_each(|x| F::write_int(w, *x));
        }
        Value::LongArray(a) => {
            F::write_int(w, a.len() as i32);
            a.iter().for_each(|x| F::write_long(w, *x));
        }
    }
}
