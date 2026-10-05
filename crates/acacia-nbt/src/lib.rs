//! Minecraft: Bedrock Edition NBT in its two wire flavours:
//! - [`Network`]: little-endian with varint lengths and zigzag ints (minecraft-data `nbt`).
//! - [`LittleEndian`]: plain little-endian (minecraft-data `lnbt`; item extra data, world storage).
//!
//! Limits on hostile input and what [`write`] expects of a tree: see README.md.

mod error;
mod raw;
mod read;
mod skip;
mod wire;
mod write;

pub use error::Error;
pub use raw::Raw;
pub use read::{read, read_lossy};
pub use skip::skip;
pub use wire::{Flavor, LittleEndian, Network};
pub use write::write;

pub type Result<T> = std::result::Result<T, Error>;

pub const MAX_DEPTH: usize = 512;

/// Keys and string values: stored inline up to 24 bytes, which covers most of them (README.md, "Performance").
pub type Str = compact_str::CompactString;

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
    String(Str),
    List(List),
    /// Entries keep wire order so re-encoding is byte-exact.
    Compound(Vec<(Str, Value)>),
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
    /// Almost always empty, which a `String` holds without allocating.
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
