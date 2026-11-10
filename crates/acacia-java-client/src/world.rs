//! What a client knows of the level: chunks with Java's native state ids, and entities.

use std::collections::HashMap;

use crate::chunks::Chunks;
use crate::entity::{MOB_AGGRESSIVE, has_mob_flags, is_rocket, read_mob_flags, read_rocket_target};
pub use crate::entity::{Entity, Mover};

use acacia_java_physics::{Aabb, Placement};
use crate::movement::placement;
use acacia_java_proto::actions::BlockPos;
use acacia_java_proto::chunk::{read_chunk_header, read_sections};
use acacia_java_proto::entity::{PositionPath, read_lp_vec3, read_move, read_path, skip_lp_vec3};
use acacia_java_proto::packets::{PositionMoveRotation, SetPassengers};
use acacia_java_proto::{Error, VarInt, Wire, ids, read_varint, read_varlong};

use ids::play::clientbound as from;

/// A byte angle in degrees.
fn degrees(angle: i8) -> f32 {
    f32::from(angle) * 360.0 / 256.0
}

#[derive(Debug, Default)]
pub struct World {
    pub dimension: String,
    pub player_id: i32,
    chunks: Chunks,
    pub entities: HashMap<i32, Entity>,
    /// The vehicle the player drives: the client simulates it, so the server's moves only set its codec base.
    pub(crate) driven: Option<i32>,
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

    /// Boxes of the entities that block `mover`, minus `skip` (the mover and whoever rides with it).
    pub fn collision_boxes(&self, mover: Mover, skip: &[i32]) -> Vec<Aabb> {
        self.entities.iter().filter(|(id, _)| !skip.contains(id)).filter_map(|(_, e)| e.collision_box(mover)).collect()
    }

    /// Writes a block ahead of the server, as vanilla's prediction does; the server's update overwrites it.
    pub fn set_block(&mut self, pos: [i32; 3], state: u32) {
        self.chunks.set_block(pos, state);
    }

    /// Each entity's `interpolate()`, as `tickEntities` runs it once a client tick. An unridden boat's tick zeroes
    /// its motion too.
    pub fn tick_entities(&mut self) {
        for e in self.entities.values_mut() {
            e.lerp.interpolate(&mut e.position);
            e.motion = [0.0; 3];
        }
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
                let path = read_path(input)?;
                let (y_rot, x_rot, on_ground) = (f32::read(input)?, f32::read(input)?, bool::read(input)?);
                let driven = self.driven == Some(id);
                if let Some(e) = self.entities.get_mut(&id) {
                    e.base = path.end();
                    if !driven {
                        e.move_to(Some(path), Some((y_rot, x_rot)), true);
                        e.on_ground = on_ground;
                    }
                }
            }
            from::MOVE_ENTITY_POS | from::MOVE_ENTITY_POS_ROT => {
                let id2 = read_varint(input)?;
                let driven = self.driven == Some(id2);
                let Some(e) = self.entities.get_mut(&id2) else { return Ok(()) };
                let (on_ground, path) = read_move(input, e.base)?;
                let rot = if id == from::MOVE_ENTITY_POS_ROT { Some((degrees(i8::read(input)?), degrees(i8::read(input)?))) } else { None };
                e.base = path.end();
                if !driven {
                    e.move_to(Some(path), rot, false);
                    e.on_ground = on_ground;
                }
            }
            from::SET_ENTITY_MOTION => {
                let id = read_varint(input)?;
                if let Some(e) = self.entities.get_mut(&id) {
                    e.motion = read_lp_vec3(input)?;
                }
            }
            from::SET_PASSENGERS => {
                let SetPassengers { vehicle, passengers } = SetPassengers::read(input)?;
                if let Some(e) = self.entities.get_mut(&vehicle.0) {
                    e.passengers = passengers.iter().map(|p| p.0).collect();
                }
            }
            from::SET_ENTITY_DATA => {
                let id = read_varint(input)?;
                let Some(e) = self.entities.get_mut(&id) else { return Ok(()) };
                e.read_body_data(input)?;
                if is_rocket(e.kind) {
                    read_rocket_target(input)?.inspect(|&target| e.attached_to = target);
                } else if has_mob_flags(e.kind) {
                    read_mob_flags(input)?.inspect(|flags| e.aggressive = flags & MOB_AGGRESSIVE != 0);
                }
            }
            from::MOVE_ENTITY_ROT => {
                let entity = read_varint(input)?;
                let on_ground = bool::read(input)?;
                let (y_rot, x_rot) = (degrees(i8::read(input)?), degrees(i8::read(input)?));
                if let Some(e) = self.entities.get_mut(&entity).filter(|_| self.driven != Some(entity)) {
                    e.move_to(None, Some((y_rot, x_rot)), false);
                    e.on_ground = on_ground;
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
        self.entities.insert(id, Entity::new(uuid, kind, position, y_rot, x_rot));
        Ok(())
    }

    fn teleport_entity(&mut self, input: &mut &[u8]) -> Result<(), Error> {
        let id = read_varint(input)?;
        let change = placement(&PositionMoveRotation::read(input)?);
        let relatives = i32::read(input)?;
        let on_ground = bool::read(input)?;
        // The driven vehicle's teleport goes through `Client::vehicle_teleported`.
        let Some(e) = self.entities.get_mut(&id).filter(|_| self.driven != Some(id)) else { return Ok(()) };
        let source = Placement { pos: e.position, delta: e.motion, y_rot: e.y_rot, x_rot: e.x_rot };
        let to = source.teleported(&change, relatives);
        e.motion = to.delta;
        e.move_to(Some(PositionPath::Linear(to.pos)), Some((to.y_rot, to.x_rot)), true);
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
