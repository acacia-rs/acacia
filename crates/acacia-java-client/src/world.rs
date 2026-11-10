//! What a client knows of the level: chunks with Java's native state ids, and entities.

use std::collections::HashMap;

use crate::chunks::Chunks;

use acacia_java_proto::actions::BlockPos;
use acacia_java_proto::chunk::{read_chunk_header, read_sections};
use acacia_java_proto::entity::{read_move, read_path_end, skip_lp_vec3};
use acacia_java_proto::packets::PositionMoveRotation;
use acacia_java_proto::{Error, VarInt, Wire, ids, read_varint, read_varlong};

use ids::play::clientbound as from;

/// A byte angle in degrees.
fn degrees(angle: i8) -> f32 {
    f32::from(angle) * 360.0 / 256.0
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub uuid: u128,
    /// `entity_type` registry id.
    pub kind: i32,
    pub position: [f64; 3],
    pub y_rot: f32,
    pub x_rot: f32,
    pub on_ground: bool,
}

#[derive(Debug, Default)]
pub struct World {
    pub dimension: String,
    pub player_id: i32,
    chunks: Chunks,
    pub entities: HashMap<i32, Entity>,
}

/// Lowest y of a vanilla dimension. The client claims the vanilla packs, so a custom dimension type would be
/// unknown here; it is treated as the overworld.
fn min_y(dimension: &str) -> i32 {
    match dimension {
        "minecraft:the_nether" | "minecraft:the_end" => 0,
        _ => -64,
    }
}

impl World {
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// The loaded blocks; clone it for a snapshot.
    pub fn chunks(&self) -> &Chunks {
        &self.chunks
    }

    /// The state id at a position, `None` outside the loaded sections.
    pub fn block(&self, pos: [i32; 3]) -> Option<u32> {
        self.chunks.block(pos)
    }

    pub fn min_y(&self) -> i32 {
        self.chunks.min_y()
    }

    pub(crate) fn set_block(&mut self, pos: [i32; 3], state: u32) {
        self.chunks.set_block(pos, state);
    }

    fn enter_level(&mut self, dimension: String) {
        self.chunks = Chunks::new(min_y(&dimension));
        self.dimension = dimension;
        self.entities.clear();
    }

    /// Updates the world from a play packet; ones it has no use for are ignored.
    pub fn apply(&mut self, id: i32, mut input: &[u8]) -> Result<(), Error> {
        let input = &mut input;
        match id {
            from::LOGIN => self.login(input)?,
            from::RESPAWN => {
                read_varint(input)?;
                self.enter_level(String::read(input)?);
            }
            from::LEVEL_CHUNK_WITH_LIGHT => {
                let (x, z, buffer) = read_chunk_header(input)?;
                self.chunks.insert(x, z, read_sections(&buffer)?);
            }
            from::FORGET_LEVEL_CHUNK => {
                let packed = i64::read(input)?;
                self.chunks.remove(packed as i32, (packed >> 32) as i32);
            }
            from::BLOCK_UPDATE => {
                let BlockPos(pos) = BlockPos::read(input)?;
                self.set_block(pos, read_varint(input)? as u32);
            }
            from::SECTION_BLOCKS_UPDATE => {
                let [sx, sy, sz] = unpack_section_pos(i64::read(input)?);
                for _ in 0..read_varint(input)? {
                    let change = read_varlong(input)? as u64;
                    let p = (change & 0xfff) as i32;
                    let pos = [sx * 16 + (p >> 8 & 15), sy * 16 + (p & 15), sz * 16 + (p >> 4 & 15)];
                    self.set_block(pos, (change >> 12) as u32);
                }
            }
            from::ADD_ENTITY => self.add_entity(input)?,
            from::REMOVE_ENTITIES => <Vec<VarInt>>::read(input)?.iter().for_each(|id| {
                self.entities.remove(&id.0);
            }),
            from::TELEPORT_ENTITY => self.teleport_entity(input)?,
            from::ENTITY_POSITION_SYNC => {
                let id = read_varint(input)?;
                let position = read_path_end(input)?;
                let (y_rot, x_rot, on_ground) = (f32::read(input)?, f32::read(input)?, bool::read(input)?);
                if let Some(e) = self.entities.get_mut(&id) {
                    (e.position, e.y_rot, e.x_rot, e.on_ground) = (position, y_rot, x_rot, on_ground);
                }
            }
            from::MOVE_ENTITY_POS | from::MOVE_ENTITY_POS_ROT => {
                let id2 = read_varint(input)?;
                let Some(e) = self.entities.get_mut(&id2) else { return Ok(()) };
                let (on_ground, position) = read_move(input, e.position)?;
                (e.on_ground, e.position) = (on_ground, position);
                if id == from::MOVE_ENTITY_POS_ROT {
                    (e.y_rot, e.x_rot) = (degrees(i8::read(input)?), degrees(i8::read(input)?));
                }
            }
            from::MOVE_ENTITY_ROT => {
                let entity = read_varint(input)?;
                let on_ground = bool::read(input)?;
                let (y_rot, x_rot) = (degrees(i8::read(input)?), degrees(i8::read(input)?));
                if let Some(e) = self.entities.get_mut(&entity) {
                    (e.on_ground, e.y_rot, e.x_rot) = (on_ground, y_rot, x_rot);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn login(&mut self, input: &mut &[u8]) -> Result<(), Error> {
        self.player_id = i32::read(input)?;
        bool::read(input)?;
        <Vec<String>>::read(input)?;
        for _ in 0..3 {
            read_varint(input)?;
        }
        for _ in 0..3 {
            bool::read(input)?;
        }
        read_varint(input)?;
        self.enter_level(String::read(input)?);
        Ok(())
    }

    fn add_entity(&mut self, input: &mut &[u8]) -> Result<(), Error> {
        let id = read_varint(input)?;
        let uuid = u128::read(input)?;
        let kind = read_varint(input)?;
        let position = [f64::read(input)?, f64::read(input)?, f64::read(input)?];
        skip_lp_vec3(input)?;
        let (x_rot, y_rot) = (degrees(i8::read(input)?), degrees(i8::read(input)?));
        self.entities.insert(id, Entity { uuid, kind, position, y_rot, x_rot, on_ground: false });
        Ok(())
    }

    /// `Relative` bits as in [`crate::Pose`].
    fn teleport_entity(&mut self, input: &mut &[u8]) -> Result<(), Error> {
        let id = read_varint(input)?;
        let c = PositionMoveRotation::read(input)?;
        let relatives = i32::read(input)?;
        let on_ground = bool::read(input)?;
        let Some(e) = self.entities.get_mut(&id) else { return Ok(()) };
        let is = |bit: u32| relatives & (1 << bit) != 0;
        let [x, y, z] = e.position;
        e.position = [c.x + if is(0) { x } else { 0.0 }, c.y + if is(1) { y } else { 0.0 }, c.z + if is(2) { z } else { 0.0 }];
        e.y_rot = c.y_rot + if is(3) { e.y_rot } else { 0.0 };
        e.x_rot = c.x_rot + if is(4) { e.x_rot } else { 0.0 };
        e.on_ground = on_ground;
        Ok(())
    }
}

/// `SectionPos.asLong`: x 22 bits, z 22, y 20.
fn unpack_section_pos(v: i64) -> [i32; 3] {
    [(v >> 42) as i32, (v << 44 >> 44) as i32, (v << 22 >> 42) as i32]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_sections_keep_their_sign() {
        let section = |x: i64, y: i64, z: i64| (x & 0x3f_ffff) << 42 | (z & 0x3f_ffff) << 20 | (y & 0xf_ffff);
        assert_eq!(unpack_section_pos(section(-3, -4, 9)), [-3, -4, 9]);
    }
}
