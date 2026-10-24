//! Reads back the chunk NBT `encode.rs` writes (blocks and biomes), so a column evicted from memory can
//! serve as a neighbour again (store/disk.rs). Light is not read: it follows from the blocks.

use crate::column::{JavaColumn, SectionBiomes};
use crate::mapping::{self, JavaState};
use crate::nbt;
use crate::pack::{biome_bits, block_bits, unpack};
use crate::section::{Blocks, VOLUME};

#[derive(Debug, thiserror::Error)]
#[error("unreadable chunk NBT: {0}")]
pub struct DecodeError(&'static str);

enum Tag {
    Int(i32),
    String(String),
    List(Vec<Tag>),
    Compound(Vec<(String, Tag)>),
    LongArray(Vec<u64>),
    Other,
}

impl Tag {
    fn get(&self, key: &str) -> Option<&Tag> {
        match self {
            Tag::Compound(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn int(&self, key: &str) -> Result<i32, DecodeError> {
        match self.get(key) {
            Some(Tag::Int(v)) => Ok(*v),
            _ => Err(DecodeError("missing int")),
        }
    }

    fn list(&self, key: &str) -> Result<&[Tag], DecodeError> {
        match self.get(key) {
            Some(Tag::List(v)) => Ok(v),
            _ => Err(DecodeError("missing list")),
        }
    }

    fn longs(&self) -> &[u64] {
        match self.get("data") {
            Some(Tag::LongArray(v)) => v,
            _ => &[],
        }
    }

    fn str(&self) -> Result<&str, DecodeError> {
        match self {
            Tag::String(s) => Ok(s),
            _ => Err(DecodeError("not a string")),
        }
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], DecodeError> {
        if self.0.len() < n {
            return Err(DecodeError("truncated"));
        }
        let (head, rest) = self.0.split_at(n);
        self.0 = rest;
        Ok(head)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    fn i32(&mut self) -> Result<i32, DecodeError> {
        Ok(i32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn len(&mut self) -> Result<usize, DecodeError> {
        usize::try_from(self.i32()?).map_err(|_| DecodeError("negative length"))
    }

    fn string(&mut self) -> Result<String, DecodeError> {
        let n = u16::from_be_bytes(self.take(2)?.try_into().unwrap()) as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }

    fn payload(&mut self, tag: u8, depth: usize) -> Result<Tag, DecodeError> {
        if depth > 16 {
            return Err(DecodeError("too deep"));
        }
        Ok(match tag {
            nbt::BYTE => {
                self.take(1)?;
                Tag::Other
            }
            2 => {
                self.take(2)?;
                Tag::Other
            }
            nbt::INT => Tag::Int(self.i32()?),
            5 => {
                self.take(4)?;
                Tag::Other
            }
            nbt::LONG | 6 => {
                self.take(8)?;
                Tag::Other
            }
            nbt::BYTE_ARRAY => {
                let n = self.len()?;
                self.take(n)?;
                Tag::Other
            }
            nbt::STRING => Tag::String(self.string()?),
            nbt::LIST => {
                let elem = self.u8()?;
                let n = self.len()?;
                let mut items = Vec::with_capacity(n.min(1024));
                for _ in 0..n {
                    items.push(self.payload(elem, depth + 1)?);
                }
                Tag::List(items)
            }
            nbt::COMPOUND => {
                let mut fields = Vec::new();
                loop {
                    let t = self.u8()?;
                    if t == nbt::END {
                        break;
                    }
                    let name = self.string()?;
                    fields.push((name, self.payload(t, depth + 1)?));
                }
                Tag::Compound(fields)
            }
            nbt::INT_ARRAY => {
                let n = self.len()?;
                self.take(n.checked_mul(4).ok_or(DecodeError("length"))?)?;
                Tag::Other
            }
            nbt::LONG_ARRAY => {
                let n = self.len()?;
                let raw = self.take(n.checked_mul(8).ok_or(DecodeError("length"))?)?;
                Tag::LongArray(
                    raw.as_chunks::<8>()
                        .0
                        .iter()
                        .map(|c| u64::from_be_bytes(*c))
                        .collect(),
                )
            }
            _ => return Err(DecodeError("unknown tag")),
        })
    }
}

/// A column from its chunk NBT (as stored in a region slot, decompressed).
pub fn decode_chunk(bytes: &[u8]) -> Result<JavaColumn, DecodeError> {
    let mut r = Reader(bytes);
    if r.u8()? != nbt::COMPOUND {
        return Err(DecodeError("root is not a compound"));
    }
    r.string()?;
    let root = r.payload(nbt::COMPOUND, 0)?;
    let min_section = root.int("yPos")?;
    let mut sections = Vec::new();
    let mut biomes = Vec::new();
    let mut idx = vec![0u16; VOLUME];
    let mut states = Box::new([0 as JavaState; VOLUME]);
    for s in root.list("sections")? {
        let bs = s.get("block_states").ok_or(DecodeError("no block_states"))?;
        let palette = bs
            .list("palette")?
            .iter()
            .map(state)
            .collect::<Result<Vec<_>, _>>()?;
        sections.push(match palette.as_slice() {
            [] => return Err(DecodeError("empty palette")),
            [one] => Blocks::Uniform(*one),
            _ => {
                if !unpack(bs.longs(), block_bits(palette.len()), &mut idx) {
                    return Err(DecodeError("short block data"));
                }
                for (o, &p) in states.iter_mut().zip(&idx) {
                    *o = *palette.get(p as usize).ok_or(DecodeError("index"))?;
                }
                Blocks::from_full(&states)
            }
        });
        biomes.push(section_biomes(s, &mut idx[..64])?);
    }
    Ok(JavaColumn {
        x: root.int("xPos")?,
        z: root.int("zPos")?,
        min_section,
        sections,
        biomes,
    })
}

fn state(entry: &Tag) -> Result<JavaState, DecodeError> {
    let name = entry.get("id").ok_or(DecodeError("state without id"))?.str()?;
    let mut props = Vec::new();
    if let Some(Tag::Compound(fields)) = entry.get("properties") {
        for (k, v) in fields {
            props.push((k.as_str(), v.str()?));
        }
    }
    mapping::java()
        .state(name, &props)
        .ok_or(DecodeError("unknown state"))
}

fn section_biomes(section: &Tag, idx: &mut [u16]) -> Result<SectionBiomes, DecodeError> {
    let b = section.get("biomes").ok_or(DecodeError("no biomes"))?;
    let table = mapping::biomes();
    let palette = b
        .list("palette")?
        .iter()
        .map(|t| table.index(t.str()?).ok_or(DecodeError("unknown biome")))
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = [0u16; 64];
    match palette.as_slice() {
        [] => return Err(DecodeError("empty biome palette")),
        [one] => out.fill(*one),
        _ => {
            if !unpack(b.longs(), biome_bits(palette.len()), idx) {
                return Err(DecodeError("short biome data"));
            }
            for (o, &p) in out.iter_mut().zip(idx.iter()) {
                *o = *palette.get(p as usize).ok_or(DecodeError("index"))?;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
