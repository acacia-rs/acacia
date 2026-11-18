//! `LevelChunkWithLight`, `LightUpdate`, `ForgetLevelChunk`: the chunk data a client renders and drops.

use super::heightmap::Heightmap;
use super::light::LightData;
use super::sections::{Widths, write_sections};
use crate::chunk::{Section, read_sections};
use crate::tag::Tag;
use crate::types::ChunkPos;
use crate::{Error, VarInt, Wire};

/// `ByteBufCodecs.byteArray(2097152)` bound of the section buffer.
const MAX_BUFFER: usize = 2 * 1024 * 1024;

/// `ClientboundLevelChunkPacketData.BlockEntityInfo`: the position inside the chunk, the type's registry id and the
/// update tag; an empty tag is sent as none (a single 0 byte).
#[derive(Debug, Clone, PartialEq)]
pub struct BlockEntity {
    /// Block coordinates inside the chunk, 0..16.
    pub x: u8,
    pub z: u8,
    pub y: i16,
    pub kind: VarInt,
    pub tag: Option<Tag>,
}

impl BlockEntity {
    /// `x` and `z` in any coordinates; only their low four bits are kept.
    pub fn new(x: i32, y: i32, z: i32, kind: i32, tag: Option<Tag>) -> Self {
        Self { x: (x & 15) as u8, z: (z & 15) as u8, y: y as i16, kind: VarInt(kind), tag }
    }
}

impl Wire for BlockEntity {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let packed = u8::read(input)?;
        let (y, kind) = (i16::read(input)?, VarInt::read(input)?);
        Ok(Self { x: packed >> 4, z: packed & 15, y, kind, tag: Tag::read_root(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        ((self.x & 15) << 4 | (self.z & 15)).write(out);
        self.y.write(out);
        self.kind.write(out);
        match &self.tag {
            Some(tag) => tag.write_root(out),
            None => out.push(crate::nbt::TAG_END),
        }
    }
}

/// `ClientboundLevelChunkPacketData`. `buffer` is the encoded sections (see [`write_sections`]), kept encoded so a
/// server can build it once and send it to many clients.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChunkData {
    pub heightmaps: Vec<Heightmap>,
    pub buffer: Vec<u8>,
    pub block_entities: Vec<BlockEntity>,
}

impl ChunkData {
    pub fn with_sections(heightmaps: Vec<Heightmap>, sections: &[Section], widths: Widths, block_entities: Vec<BlockEntity>) -> Self {
        let mut buffer = Vec::new();
        write_sections(sections, widths, &mut buffer);
        Self { heightmaps, buffer, block_entities }
    }

    pub fn sections(&self) -> Result<Vec<Section>, Error> {
        read_sections(&self.buffer)
    }
}

impl Wire for ChunkData {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let heightmaps = Vec::read(input)?;
        let buffer = <Vec<u8>>::read(input)?;
        if buffer.len() > MAX_BUFFER {
            return Err(Error::Invalid("chunk buffer size"));
        }
        Ok(Self { heightmaps, buffer, block_entities: Vec::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.heightmaps.write(out);
        self.buffer.write(out);
        self.block_entities.write(out);
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LevelChunkWithLight {
    pub x: i32,
    pub z: i32,
    pub chunk: ChunkData,
    pub light: LightData,
}

impl Wire for LevelChunkWithLight {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { x: i32::read(input)?, z: i32::read(input)?, chunk: ChunkData::read(input)?, light: LightData::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.x.write(out);
        self.z.write(out);
        self.chunk.write(out);
        self.light.write(out);
    }
}

/// Note the VarInt coordinates, unlike the chunk packet's ints.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LightUpdate {
    pub x: VarInt,
    pub z: VarInt,
    pub light: LightData,
}

impl Wire for LightUpdate {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { x: VarInt::read(input)?, z: VarInt::read(input)?, light: LightData::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.x.write(out);
        self.z.write(out);
        self.light.write(out);
    }
}

/// `ChunkPos.pack`: x in the low 32 bits, z in the high; azalea gets negative x wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ForgetLevelChunk {
    pub pos: ChunkPos,
}

impl Wire for ForgetLevelChunk {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Ok(Self { pos: ChunkPos::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.pos.write(out);
    }
}
