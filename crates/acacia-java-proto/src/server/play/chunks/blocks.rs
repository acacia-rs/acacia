//! Single-block packets: updates, block entity data, block events, break progress and the placement ack.

#![allow(unused_doc_comments)] // docs on `packet!` fields' structs

use super::VarLong;
use crate::actions::BlockPos;
use crate::packets::packet;
use crate::tag::Tag;
use crate::types::{SectionBlockPos, SectionPos};
use crate::{Error, VarInt, Wire};

/// `state` is a block state registry id.
packet!(BlockUpdate { pos: BlockPos, state: VarInt });
packet!(BlockChangedAck { sequence: VarInt });
/// `block` is a block registry id; `b0` and `b1` are the event's two bytes.
packet!(BlockEvent { pos: BlockPos, b0: u8, b1: u8, block: VarInt });
/// `progress` 0..=9 draws the cracks; any other value clears them.
packet!(BlockDestruction { id: VarInt, pos: BlockPos, progress: u8 });

/// `ClientboundBlockEntityDataPacket`: `kind` is a block entity type registry id, the tag a nameless compound.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockEntityData {
    pub pos: BlockPos,
    pub kind: VarInt,
    pub tag: Tag,
}

impl Wire for BlockEntityData {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let (pos, kind) = (BlockPos::read(input)?, VarInt::read(input)?);
        let tag = Tag::read_root(input)?.ok_or(Error::Invalid("block entity tag"))?;
        Ok(Self { pos, kind, tag })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.pos.write(out);
        self.kind.write(out);
        self.tag.write_root(out);
    }
}

/// One changed block: its place in the section and its new state id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockChange {
    pub pos: SectionBlockPos,
    pub state: u32,
}

/// `ClientboundSectionBlocksUpdatePacket`: the section, then a VarLong each of `state << 12 | x << 8 | z << 4 | y`.
/// 26.3 carries no light flag.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SectionBlocksUpdate {
    pub section: SectionPos,
    pub changes: Vec<BlockChange>,
}

impl Wire for SectionBlocksUpdate {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let section = SectionPos::read(input)?;
        let count = VarInt::read(input)?.0;
        let mut changes = Vec::with_capacity((count.max(0) as usize).min(4096));
        for _ in 0..count {
            let v = VarLong::read(input)?.0 as u64;
            let pos = SectionBlockPos { x: (v >> 8 & 15) as u8, z: (v >> 4 & 15) as u8, y: (v & 15) as u8 };
            changes.push(BlockChange { pos, state: (v >> 12) as u32 });
        }
        Ok(Self { section, changes })
    }

    fn write(&self, out: &mut Vec<u8>) {
        self.section.write(out);
        VarInt(self.changes.len() as i32).write(out);
        for change in &self.changes {
            VarLong(i64::from(change.state) << 12 | i64::from(change.pos.packed() as u16)).write(out);
        }
    }
}
