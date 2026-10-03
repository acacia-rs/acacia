//! Block entities (signs, chests, banners, ...): their network NBT from chunk data and `BlockActorData`.

mod chunks;

use std::collections::HashMap;

use acacia_client::proto::nbt::{Nbt, Value};
use acacia_client::proto::packets::{
    BlockEntityData, ChangeDimension, LevelChunk, NetworkChunkPublisherUpdate, StartGame, Subchunk, UpdateBlock, UpdateBlockSynced,
    UpdateSubchunkBlocks,
};
use acacia_client::proto::types::BlockCoordinates;
use acacia_client::proto::{DecodeError, Packet, RawPacket};

/// Which block entities are kept from chunk data. Signs sent in `BlockActorData` are always kept.
///
/// Cost: physics bots decode every section anyway, so finding the block entities after them is
/// small next to it. Idle bots decode no terrain: with [`Self::Signs`] they skip the sections
/// (headers and palettes are read, packed block words jumped over) and parse only the NBT after
/// them; with the blob cache a sub-chunk entry holds nothing but block entities. On sub-chunk
/// request servers (BDS) idle bots only receive what vanilla requests (columns within √17 chunks).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BlockEntityTracking {
    /// [`Self::All`] on physics bots, [`Self::Off`] on idle bots.
    #[default]
    WithTerrain,
    Off,
    /// Only signs, so [`crate::Bot::write_sign`] keeps the side it does not edit.
    Signs,
    All,
}

impl BlockEntityTracking {
    /// Replaces [`Self::WithTerrain`] by what it means for this bot.
    pub fn resolve(self, physics: bool) -> Self {
        match self {
            Self::WithTerrain if physics => Self::All,
            Self::WithTerrain => Self::Off,
            other => other,
        }
    }

    pub fn packets(self) -> &'static [u32] {
        match self {
            Self::Signs | Self::All => &[
                BlockEntityData::ID,
                ChangeDimension::ID,
                StartGame::ID,
                LevelChunk::ID,
                Subchunk::ID,
                UpdateBlock::ID,
                UpdateBlockSynced::ID,
                UpdateSubchunkBlocks::ID,
                NetworkChunkPublisherUpdate::ID,
            ],
            Self::WithTerrain | Self::Off => &[BlockEntityData::ID, ChangeDimension::ID],
        }
    }
}

type Column = HashMap<[i32; 3], Nbt>;

/// Block entities by position, grouped by chunk column.
#[derive(Debug, Default)]
pub struct BlockEntities {
    tracking: BlockEntityTracking,
    /// Air's id in block updates (from StartGame); an update to air removes the block entity.
    air: Option<u32>,
    columns: HashMap<(i32, i32), Column>,
}

impl BlockEntities {
    pub(crate) fn new(tracking: BlockEntityTracking) -> Self {
        Self { tracking, ..Self::default() }
    }

    pub fn tracking(&self) -> BlockEntityTracking {
        self.tracking
    }

    pub fn apply(&mut self, packet: &RawPacket) -> Result<(), DecodeError> {
        match packet.id {
            BlockEntityData::ID => {
                let p: BlockEntityData = packet.decode()?;
                if self.keeps(&p.nbt) {
                    self.insert(coords(p.position), p.nbt);
                }
            }
            ChangeDimension::ID => self.columns.clear(),
            StartGame::ID => self.air = Some(crate::world::air_wire_id(&packet.decode()?)),
            LevelChunk::ID => self.level_chunk(packet.decode()?),
            Subchunk::ID => self.sub_chunk(packet.decode()?),
            NetworkChunkPublisherUpdate::ID => {
                let p: NetworkChunkPublisherUpdate = packet.decode()?;
                let (cx, cz, radius) = (p.coordinates.x >> 4, p.coordinates.z >> 4, (p.radius as i32 >> 4) + 1);
                self.columns.retain(|&(x, z), _| (x - cx).abs() <= radius && (z - cz).abs() <= radius);
            }
            _ if self.columns.is_empty() => {}
            UpdateBlock::ID => {
                let u: UpdateBlock = packet.decode()?;
                self.on_block_update(u.position, u.layer, u.block_runtime_id);
            }
            UpdateBlockSynced::ID => {
                let u: UpdateBlockSynced = packet.decode()?;
                self.on_block_update(u.position, u.layer, u.block_runtime_id);
            }
            UpdateSubchunkBlocks::ID => {
                for b in packet.decode::<UpdateSubchunkBlocks>()?.blocks {
                    self.on_block_update(b.position, 0, b.runtime_id);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// The block entity's NBT at `pos`, if the server sent it and it is kept.
    pub fn get(&self, pos: [i32; 3]) -> Option<&Nbt> {
        self.columns.get(&column(pos))?.get(&pos)
    }

    /// The sign (or hanging sign) block entity at `pos`.
    pub fn sign(&self, pos: [i32; 3]) -> Option<&Nbt> {
        self.get(pos).filter(|n| is_sign(n))
    }

    /// The text of one side of the sign at `pos` (lines joined by `\n`), if known.
    pub fn sign_text(&self, pos: [i32; 3], front: bool) -> Option<&str> {
        let side = self.sign(pos)?.value.get(if front { "FrontText" } else { "BackText" })?;
        match side.get("Text") {
            Some(Value::String(text)) => Some(text),
            _ => None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&[i32; 3], &Nbt)> {
        self.columns.values().flatten()
    }

    pub fn len(&self) -> usize {
        self.columns.values().map(HashMap::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.values().all(HashMap::is_empty)
    }

    pub(crate) fn insert(&mut self, pos: [i32; 3], nbt: Nbt) {
        self.columns.entry(column(pos)).or_default().insert(pos, nbt);
    }

    fn keeps(&self, nbt: &Nbt) -> bool {
        self.tracking == BlockEntityTracking::All || is_sign(nbt)
    }

    /// Only updates to air remove: servers resend the clicked block after a rejected use, which
    /// must not drop the sign about to be written.
    fn on_block_update(&mut self, p: BlockCoordinates, layer: u32, wire_id: u32) {
        if layer != 0 || Some(wire_id) != self.air {
            return;
        }
        let pos = coords(p);
        if let Some(c) = self.columns.get_mut(&column(pos)) {
            c.remove(&pos);
        }
    }
}

/// `Sign` or `HangingSign` block entity data.
fn is_sign(nbt: &Nbt) -> bool {
    matches!(nbt.value.get("id"), Some(Value::String(id)) if id.ends_with("Sign"))
}

fn coords(p: BlockCoordinates) -> [i32; 3] {
    [p.x, p.y, p.z]
}

fn column([x, _, z]: [i32; 3]) -> (i32, i32) {
    (x >> 4, z >> 4)
}

#[cfg(test)]
mod tests;
